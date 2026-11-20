# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/plugins/chaos.md#Plugin-chaos
# see: cans/plugins/ledger.md#Plugin-ledger
from __future__ import annotations

import asyncio
import logging
import os
import platform
import uuid
from dataclasses import dataclass
from datetime import timedelta
from enum import Enum
from pathlib import Path
from typing import Any, Awaitable, Callable, Literal, Optional, Self

import kelir
from kelir import (
    BaseModel,
    CausalDelta,
    Errno,
    ExecutionReceipt,
    External,
    Field,
    FilterAction,
    HandlerContext,
    KelirBuilder,
    PatchPayload,
    PostExecAction,
    Session,
    SnapshotDescriptor,
    SpawnResult,
    TrapEvent,
    TrapFilter,
    Verdict,
    WatchdogAction,
)
from kelir.plugin_model import LifecycleCtx
from kelir.plugins.redaction import (
    MaskReadConfig, MaskReadRule, RedactionConfig, redaction,
)
from kelir.plugins.terminal_guard import (
    PromptRuleConfig, TerminalGuardConfig, terminal_guard,
)
from kelir.plugins.token_shield import TokenShieldConfig, token_shield
from kelir.plugins.vault import CredentialDescriptor, VaultConfig, VaultPlugin
from kelir.storage_planes import IdmapConfig, SharedCacheConfig
from kelir.wire_planes import (
    L4Proxy, MtlsConfig, SinkholeConfig, SinkholeRule
)
from kelir.plugins.chaos import (
    ChaosConfig,
    StorageFaultRule,
    NetworkFaultRule,
    ChaosPlugin,
)
from kelir.plugins.ledger import (
    LedgerConfig,
    LedgerPlugin,
    OtelConfig,
    ReceiptConfig,
)

logging.basicConfig(level=logging.INFO, format="%(message)s")
log = logging.getLogger("kelir.examples.swe-agent")


class Env(str, Enum):
    STAGING = "staging"
    PRODUCTION = "production"


# see: cans/03-sdk-architecture.md#Python-SDK
class FixStripeWebhook(BaseModel, frozen=True):
    action: Literal["fix-stripe-webhook"]
    repo: str
    issue_id: str = Field(alias="issueId")
    env: Env

    model_config = {"populate_by_name": True, "extra": "forbid"}


@dataclass
class RepoPolicyState:
    violations: int = 0
    current_branch: str = ""


@dataclass
class RepoPolicyOptions:
    max_diff_bytes: int
    protected_branches: tuple[str, ...] = ("main", "master", "release/*")


# see: cans/02-plugin-model.md#Full-Verdict-lattice
def repo_policy_plugin(opts: RepoPolicyOptions) -> KelirBuilder:
    state = RepoPolicyState()

    def _is_protected(branch: str) -> bool:
        return any(
            branch == p or (p.endswith("*") and branch.startswith(p[:-1]))
            for p in opts.protected_branches
        )

    async def _handle_trap(event: TrapEvent) -> Optional[Verdict]:
        is_git = event.binary_path.endswith("/git") or event.binary_path == "git"
        is_push = len(event.argv) > 1 and event.argv[1] == "push"
        if not (is_git and is_push):
            return None

        if _is_protected(state.current_branch):
            state.violations += 1
            return Verdict.drop(
                "EPERM", f"EPOLICY: '{state.current_branch}' is protected."
            )

        new_branch = f"agent/{uuid.uuid4().hex[:8]}"
        return Verdict.patch(
            binary="/bin/git",
            argv=[*event.argv, f"HEAD:refs/heads/{new_branch}"],
            env={"KELIR_REWRITTEN": "1"},
        )

    def _handle_write(
        path: Path, data: bytes, pid: int, ctx: HandlerContext,
    ) -> FilterAction:
        if path.suffix == ".diff" and len(data) > opts.max_diff_bytes:
            state.violations += 1
            return FilterAction.drop(
                errno=Errno.ENOSPC, reason="diff exceeds policy ceiling"
            )
        return FilterAction.forward()

    def _handle_unlink(path: Path, ctx: HandlerContext) -> Verdict:
        path_str = str(path)
        # Intercept and quarantine destructive deletion of lockfiles or test harnesses
        if path_str.endswith("package.json") or "test" in path_str:
            filename = path.name
            return Verdict.patch(PatchPayload.fs_path_redirect(
                target_path=f"/workspace/.agentsh_trash/{filename}"
            ))
        return Verdict.pass_()

    return (
        KelirBuilder.plugin("repo-policy")
        .require("ledger")
        .fs(lambda f: f.on_fs_write(_handle_write).on_fs_unlink(_handle_unlink))
        .trap(lambda t: t.on_trap(_handle_trap))
    )




# see: cans/02-plugin-model.md#Unified-verdict
PreExecHandler = Callable[[HandlerContext], Awaitable[Verdict]]
PostExecHandler = Callable[[HandlerContext], Awaitable[PostExecAction]]


def make_universal_step(oracle: UserOracle) -> PreExecHandler:
    async def universal_step(c: HandlerContext) -> Verdict:
        await c.checkpoint.create(f"pre:{c.trap.pid}")
        intent = await oracle.analyze_intent(c.trap)
        if not intent.is_hypothesis_valid:
            return Verdict.drop(
                "EPERM",
                f"Verification Reject: {intent.falsification_reason or ''}",
            )
        return Verdict.pass_()
    return universal_step


def make_warm_caches(oracle: UserOracle) -> PreExecHandler:
    async def warm_caches(c: HandlerContext) -> Verdict:
        await c.fs.warm_cache(["node_modules", ".next/cache"])
        # see: cans/11-protocol.md#Env-injection
        return Verdict.patch(
            binary=c.trap.binary_path,
            argv=list(c.trap.argv),
            env={"OTEL_TRACE_ID": oracle.current_trace_id(), "CI": "1"},
        )
    return warm_caches


def make_force_strip() -> PreExecHandler:
    async def force_strip(c: HandlerContext) -> Verdict:
        if "--force" in c.trap.argv:
            return Verdict.patch(
                binary="/bin/git",
                argv=[a for a in c.trap.argv if a != "--force"],
                env={"KELIR_DISARMED": "1"},
            )
        return Verdict.pass_()
    return force_strip


# see: cans/10-attestation.md#Execution-receipts
def make_verify_receipt(oracle: UserOracle) -> PostExecHandler:
    async def verify_receipt(c: HandlerContext) -> PostExecAction:
        pre: SnapshotDescriptor = c.step.last()
        receipt: ExecutionReceipt = await c.receipt()

        if receipt.binary_blake3 != c.trap.binary_blake3:
            await c.vm.rollback(pre)
            c.terminal.notice("⚡ [KELIR]: post-exec binary hash mismatch — REVERTED\n")
            return PostExecAction.rollback_and_halt()

        if (
            c.trap.binary_path.endswith("/npm")
            and receipt.egress_bytes > 0
            and "/checkout-service" in c.trap.cwd
        ):
            await c.vm.rollback(pre)
            c.terminal.notice(
                f"⚡ [KELIR]: hermetic test emitted {receipt.egress_bytes}B "
                "egress — REVERTED\n"
            )
            return PostExecAction.rollback_and_halt()

        if not await oracle.verify_physical_receipt(receipt):
            await c.vm.rollback(pre)
            c.terminal.notice("⚡ [KELIR]: post-condition violated — REVERTED\n")
            return PostExecAction.rollback_and_halt()

        return PostExecAction.proceed()
    return verify_receipt


def make_commit_gate() -> PreExecHandler:
    async def commit_gate(c: HandlerContext) -> Verdict:
        # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
        repo_policy = c.cap("repo-policy")
        n = repo_policy.violations if repo_policy else 0
        if n > 0:
            return Verdict.drop("EPERM", f"EPOLICY: {n} prior violation(s).")
        return Verdict.pass_()
    return commit_gate


# see: cans/10-attestation.md#Execution-receipts
def make_zero_byte_check() -> PostExecHandler:
    async def zero_byte_check(c: HandlerContext) -> PostExecAction:
        receipt = await c.receipt()
        if receipt.dirty_bytes == 0:
            raise RuntimeError("Verification anomaly: zero-byte delta commit attempted.")
        # see: cans/10-attestation.md#Typed-extensions
        vfs_ext = receipt.fs_vfs_ext
        if vfs_ext is None:
            log.info("[kelir] commit fs:vfs extension absent")
        else:
            log.info("[kelir] commit fs:vfs extension present, served_inodes=%s roots=%s",
                     vfs_ext.served_inodes, len(vfs_ext.roots))
        return PostExecAction.proceed()
    return zero_byte_check


async def build_runtime(
    *,
    oracle: UserOracle,
    pg_cert: Path,
    pg_key: Path,
) -> KelirBuilder:
    policy_plugin = repo_policy_plugin(
        RepoPolicyOptions(
            max_diff_bytes=64 * 1024,
            protected_branches=("main", "master", "release/*"),
        )
    )
    # see: cans/08-lifecycle.md#Watchdog
    async def _init(ctx: LifecycleCtx) -> None:
        ctx.set_watchdog(timeout=timedelta(minutes=5), action=WatchdogAction.REBOOT)

    async def _ready(ctx: LifecycleCtx | None) -> None:
        if ctx is not None:
            await ctx.exec_in_guest(["npm", "ci"])

    async def _shutdown(_ctx: LifecycleCtx | None) -> None:
        if _ctx is not None:
            ledger = _ctx.cap("ledger")
            if ledger is not None:
                await ledger.flush()

    # see: cans/08-lifecycle.md#Break-glass
    async def _deny(event: TrapEvent, _verdict: Verdict, session: Session) -> None:
        # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
        ledger = session.cap("ledger")
        if ledger is not None and await ledger.count_denials(event.ppid) > 5:
            await session.vm.suspend("./.kelir/quarantine.snap")
            await session.console()

    return (
        kelir.builder(
            vcpus=4,
            memory_mb=4096,
            kernel=f"./fixtures/vmlinux-6.6-{platform.machine()}-kelir",
            rootfs_erofs="./fixtures/rootfs-swebench.erofs",
            tsc_locked=True,
            enable_watchdog=True,
            driver="auto",
        )

        # see: cans/04-terminal-plane.md#Terminal-pipeline
        .terminal(
            cols=120,
            rows=40,
            drop_osc52=True,
            strip_title=True,
            stream_engine="StatelessRegex",
        )

        # see: cans/05-storage-plane.md#Storage-plane
        .storage(
            idmap=IdmapConfig(guest_uid=0, host_uid=os.getuid()),
            shared_caches=[
                SharedCacheConfig(host_path="~/.npm", guest_mount="/root/.npm", read_only=True),
                SharedCacheConfig(host_path="~/.cache/pip", guest_mount="/root/.cache/pip", read_only=True),
                SharedCacheConfig(host_path="~/.cargo/registry", guest_mount="/usr/local/cargo/registry", read_only=True),
            ],
        )
        .fs(
            lambda f: f.config(
                lock_broker={
                    "globs": ["**/Cargo.lock", "**/package-lock.json"],
                    "non_blocking": True,
                },
            )
        )

        # see: cans/plugins/token-shield.md#Plugin-token-shield
        .use(token_shield(TokenShieldConfig(
            max_bytes_total=128 * 1024,
            max_lines_total=500,
            clamp_strategy="head-and-tail",
            notice="⚠️ [SHIELD]",
        )))

        # see: cans/plugins/terminal-guard.md#Plugin-terminal-guard
        .use(terminal_guard(TerminalGuardConfig(
            prompts=[
                PromptRuleConfig(
                    pattern=r"(?i)Overwrite production schema\? \[y/N\]",
                    timeout_seconds=3,
                    verify_foreground=True,
                    action=list(b"y\n"),
                ),
            ],
        )))

        # see: cans/plugins/redaction.md#Plugin-redaction
        .use(redaction(RedactionConfig(
            soft_delete={"enabled": True, "trash_dir": "/workspace/.kelir_trash"},
            mask_read=MaskReadConfig(
                engine="fast-regex",
                carryover_bytes=1024,
                rules=[
                    MaskReadRule(
                        glob="**/secrets/**",
                        patterns=[
                            (r"sk_live_[0-9a-zA-Z]{24}", "[STRIPE_VAULT_KEY]"),
                            (r"AKIA[0-9A-Z]{16}", "[AWS_IAM_REDACTED]"),
                        ],
                    ),
                ],
            ),
        )))
        .use(VaultPlugin(VaultConfig(
            credsub_fake_tokens=True,
            scrub_responses=True,
            credentials=[
                CredentialDescriptor(
                    name="STRIPE_SECRET",
                    placeholder="vault://stripe_key",
                    kind="bearer-token",
                    audience="api.stripe.com",
                ),
            ],
        )))


        # see: cans/07-wire-plane.md#Vault-and-credential-flow
        .wire(
            sinkhole=SinkholeConfig(
                rules=[
                    SinkholeRule(host=host, allow=True)
                    for host in ["api.stripe.com", "github.com", "*.internal.net"]
                ]
                + [SinkholeRule(host="*", allow=False)],
            ),
            ssh_agent_bridge=True,
            l4_proxies=[
                L4Proxy(
                    listen_port=5432,
                    destination="pg.internal.net:5432",
                    inject_mtls=MtlsConfig(cert_path=pg_cert, key_path=pg_key),
                ),
            ],
            tap_filter="AllowUdpProxied",
        )
        .params(lambda p: p.model(FixStripeWebhook))

        # see: cans/plugins/chaos.md#Plugin-chaos
        .use(ChaosPlugin(ChaosConfig(
            network=[
                NetworkFaultRule(
                    target="api.stripe.com",
                    latency_ms=150,
                    jitter_ms=30,
                    drop_rate=0.02,
                ),
                NetworkFaultRule(
                    target="api.stripe.com",
                    latency_ms=150,
                    jitter_ms=30,
                    drop_rate=0.02,
                    direction="inbound",
                ),
                NetworkFaultRule(
                    target="api.stripe.com",
                    latency_ms=150,
                    jitter_ms=30,
                    drop_rate=0.02,
                    direction="both",
                ),
            ],
            storage=[
                StorageFaultRule(
                    target_glob="/workspace/build/cache/*",
                    simulate_errno="enospc",
                    probability=0.05,
                ),
            ],
        )))

        # see: cans/plugins/ledger.md#Merkle-WAL
        .use(LedgerPlugin(LedgerConfig(
            store_path=".kelir/audit.merkle.db",
            strict=True,
            ocsf_mapping=True,
            merkle_chain=True,
            otel=OtelConfig(
                endpoint="http://localhost:4318/v1/traces",
                service_name="kelir-swe-agent",
                propagate_w3c_traceparent=True,
            ),
            receipts=ReceiptConfig(
                include_syscall_count=True,
            ),
        )))
        .use(policy_plugin)
        # see: cans/02-plugin-model.md#Full-Verdict-lattice
        .trap("*", pre=make_universal_step(oracle), post=make_verify_receipt(oracle))
        .trap(TrapFilter.binary("npm").with_arg("test").with_arg("--").with_arg("--runInBand"), pre=make_warm_caches(oracle))
        .trap(TrapFilter.binary("git").with_arg("push"), pre=make_force_strip())
        .trap(TrapFilter.binary("git").with_arg("commit").with_arg("-m"), pre=make_commit_gate())
        .trap(TrapFilter.binary("git").with_arg("commit"), post=make_zero_byte_check())
        # see: cans/08-lifecycle.md#Watchdog
        .lifecycle(
            lambda l: l.on_vm_init(_init)
            .on_vm_ready(_ready)
            .on_vm_shutdown(_shutdown)
        )
        # see: cans/08-lifecycle.md#Break-glass
        .deny(lambda d: d.on_trap_deny(_deny))
        .seal()
    )


# see: cans/08-lifecycle.md#CoW-fork
async def fork_and_verify(
    parent: Session, hypothesis_id: str,
) -> tuple[str, SpawnResult, CausalDelta]:
    child = await parent.fork()
    await child.vm.clock.skew(
        timedelta(days=24 * (ord(hypothesis_id) - ord("A") + 1)),
    )
    try:
        run = await child.spawn(
            "npm",
            ["run", f"verify-hypothesis-{hypothesis_id}"],
            cwd="/workspace/checkout-service",
        )
        ledger = child.cap("ledger")
        assert ledger is not None, "ledger capability not mounted"
        delta = await ledger.get_causal_delta_from_parent()
        return hypothesis_id, run, delta
    finally:
        await child.terminate()


async def main() -> None:
    oracle = UserOracle()

    builder = await build_runtime(
        oracle=oracle,
        pg_cert=Path("/etc/ssl/client.crt"),
        pg_key=Path("/etc/ssl/client.key"),
    )

    async with builder as session:
        log.info("[kelir] booted driver=%s boot_ms=%s",
                 session.driver_name, session.boot_ms)

        await session.wire.forward_port(3000, 8080)
        base = await session.checkpoint.create("clean_base")
        winning_delta: Optional[CausalDelta] = None

        test = await session.spawn(
            "npm",
            ["test", "--", "--runInBand"],
            cwd="/workspace/checkout-service",
            params=FixStripeWebhook(
                action="fix-stripe-webhook",
                repo="checkout-service",
                issueId="ISSUE-8412",
                env=Env.STAGING,
            ),
        )
        if test.exit_code != 0:
            await session.vm.rollback(base)
            await session.console()
            return

        branches = await asyncio.gather(*(
            fork_and_verify(session, hid) for hid in "ABCD"
        ))
        # Deterministic winner evaluation in declared branch order (SSOT-A1)
        winner = next((b for target in "ABCD" for b in branches if b[0] == target and b[1].exit_code == 0), None)
        if winner is None:
            await session.vm.rollback(base)
            return
        winning_id, test_run, winning_delta = winner

        # see: cans/08-lifecycle.md#Suspend-and-resume
        await session.suspend(External("human-authorization-hold"))
        log.info("[kelir] suspended rss_mb=0")
        await oracle.await_human_approval()
        await session.resume()

        stats = await session.storage.reconcile(
            cwd="/workspace/checkout-service",
            atomic_staging=True,
            fsync=True,
        )
        log.info("[kelir] broker commit files=%s deletes=%s",
                 stats.files_changed, stats.tombstones_resolved)

        await session.spawn(
            "git",
            ["commit", "-am", f"fix(stripe): dedupe webhook body reads ({winning_id})"],
            cwd="/workspace/checkout-service",
        )

        # see: cans/10-attestation.md#Execution-receipts
        ledger = session.cap("ledger")
        assert ledger is not None, "ledger capability not mounted"
        final = await ledger.finalize(
            include_delta=winning_delta,
            include_merkle_root=True,
        )
        log.info("[kelir] receipt merkle=%s jws=%s",
                 final.merkle_root, final.signed_jws[:64])


@dataclass
class IntentAnalysis:
    is_hypothesis_valid: bool
    falsification_reason: Optional[str] = None
    allow_high_entropy: bool = False


class UserOracle:
    async def analyze_intent(self, trap: TrapEvent) -> IntentAnalysis:
        if not trap.binary_path or not trap.argv:
            return IntentAnalysis(
                is_hypothesis_valid=False,
                falsification_reason="trap carries no executable hypothesis to verify",
            )
        return IntentAnalysis(
            is_hypothesis_valid=True,
            falsification_reason=None,
            allow_high_entropy=False,
        )

    async def verify_physical_receipt(self, r: ExecutionReceipt) -> bool:
        return r.exit_kind.kind == "normal" and r.exit_code == 0

    async def ingest_harness_event(self, _p: Path, _e: dict[str, Any], _b: str) -> None:
        return None

    async def await_human_approval(self) -> None:
        await asyncio.sleep(0.05)

    def current_trace_id(self) -> str:
        return uuid.uuid4().hex


if __name__ == "__main__":
    asyncio.run(main())
