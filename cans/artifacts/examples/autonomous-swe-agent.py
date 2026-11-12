# see: cans/03-sdk-architecture.md#Python-SDK
# see: cans/plugins/chaos.md#Plugin-chaos
# see: cans/plugins/ledger.md#Plugin-ledger
# TODO: SSOT header requirements kelir>=1.0, pydantic>=2.6, blake3>=0.4 have no spec anchor; verify against packages/py/pyproject.toml (pydantic>=2.0).
from __future__ import annotations

import asyncio
import json
import logging
import os
import platform
import re
import uuid
from dataclasses import dataclass
from datetime import timedelta
from enum import Enum
from pathlib import Path
from typing import Any, Awaitable, Callable, Literal, Optional, Self

from pydantic import BaseModel, Field

import kelir
from kelir import (
    CausalDelta,
    ExecutionReceipt,
    External,
    ExternalSuspendKind,
    FilterAction,
    HandlerContext,
    KelirBuilder,
    PostExecAction,
    Session,
    SnapshotDescriptor,
    SpawnResult,
    TrapEvent,
    TrapFilter,
    Verdict,
)
from kelir.model_plugin import BasePlugin, LifecycleCtx, PluginError
from kelir.terminal_planes import PromptRule, TokenShield
from kelir.storage_planes import (
    DynamicFile, SyntheticFile, IdmapConfig, MaskRule, SnoopRule
)
from kelir.wire_planes import (
    L4Proxy, Mtls, SinkholeConfig, CredentialDescriptor
)
from kelir.plugins.plugin_chaos import (
    ChaosConfig,
    StorageFaultRule,
    NetworkFaultRule,
    ChaosPlugin,
)
from kelir.plugins.plugin_ledger import (
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


@dataclass(frozen=True)
class RepoPolicyHandle:
    _state: RepoPolicyState
    _protected: tuple[str, ...]

    @property
    def violations(self) -> int:
        return self._state.violations

    @property
    def current_branch(self) -> str:
        return self._state.current_branch

    def is_protected(self) -> bool:
        b = self._state.current_branch
        return any(
            b == p or (p.endswith("*") and b.startswith(p[:-1]))
            for p in self._protected
        )

    def record_violation(self) -> None:
        self._state.violations += 1

    def set_branch(self, b: str) -> None:
        self._state.current_branch = b


# see: cans/02-plugin-model.md#Full-Verdict-lattice
class RepoPolicyPlugin(BasePlugin):
    name = "repo-policy"
    requires: tuple[str, ...] = ("ledger",)

    def __init__(
        self,
        *,
        max_diff_bytes: int,
        protected_branches: list[str],
    ) -> None:
        self._opts: dict[str, Any] = {
            "max_diff_bytes": max_diff_bytes,
            "protected_branches": tuple(protected_branches),
        }
        self._state = RepoPolicyState()

    def handle(self) -> RepoPolicyHandle:
        return RepoPolicyHandle(
            _state=self._state,
            _protected=self._opts["protected_branches"],
        )

    async def on_trap(self, event: TrapEvent) -> Optional[Verdict]:
        is_git = event.binary_path.endswith("/git") or event.binary_path == "git"
        is_push = len(event.argv) > 1 and event.argv[1] == "push"
        if not (is_git and is_push):
            return None

        h = self.handle()
        if h.is_protected():
            h.record_violation()
            return Verdict.drop(
                "EPERM", f"EPOLICY: '{h.current_branch}' is protected."
            )

        new_branch = f"agent/{uuid.uuid4().hex[:8]}"
        return Verdict.patch(
            binary="/bin/git",
            argv=[*event.argv, f"HEAD:refs/heads/{new_branch}"],
            env={"KELIR_REWRITTEN": "1"},
        )

    def on_fs_write(self, path: Path, data: bytes, pid: int, ctx: HandlerContext) -> FilterAction:
        if path.suffix == ".diff" and len(data) > self._opts["max_diff_bytes"]:
            self.handle().record_violation()
            return FilterAction.drop(
                errno="ENOSPC", reason="diff exceeds policy ceiling"
            )
        return FilterAction.forward()

    # see: cans/04-terminal-plane.md#Terminal-pipeline
    async def disclosure_annotation(self) -> bytes:
        n = self.handle().violations
        return f"[repo-policy: {n} open violations]\n".encode()


# see: cans/02-plugin-model.md#Hook-context-restriction
def make_snoop(oracle: UserOracle) -> Callable[[Path, bytes, HandlerContext], Awaitable[None]]:
    async def on_write(path: Path, raw: bytes, c: HandlerContext) -> None:
        evt = json.loads(raw)
        if evt.get("falsifiedResult") is True:
            raise PluginError.reject(
                errno="EPERM",
                reason=f"harness hallucination at {path}",
            )
        producer_blake3 = await c.cap.ledger.blake3_of_producing_process(path)
        await oracle.ingest_harness_event(path, evt, producer_blake3)
    return on_write


# see: cans/02-plugin-model.md#Unified-verdict
PreExecHandler = Callable[[HandlerContext], Awaitable[Verdict]]
PostExecHandler = Callable[[HandlerContext], Awaitable[PostExecAction]]


def make_universal_step(oracle: UserOracle) -> PreExecHandler:
    async def universal_step(c: HandlerContext) -> Verdict:
        await c.vm.checkpoint(f"pre:{c.trap.pid}")
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
        n = c.cap.policy.violations
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
        vfs_ext = next(
            (e for e in receipt.extensions if e.namespace == "fs:vfs"),
            None,
        )
        if vfs_ext is None:
            log.info("[kelir] commit fs:vfs extension absent")
        else:
            log.info("[kelir] commit fs:vfs extension present, payload_len=%s",
                     len(vfs_ext.payload))
        return PostExecAction.proceed()
    return zero_byte_check


async def build_runtime(
    *,
    oracle: UserOracle,
    pg_cert: Path,
    pg_key: Path,
) -> KelirBuilder:
    policy_plugin = RepoPolicyPlugin(
        max_diff_bytes=64 * 1024,
        protected_branches=["main", "master", "release/*"],
    )
    builder = (
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
            token_shield=TokenShield(
                max_bytes_total=128 * 1024,
                max_lines_total=500,
                clamp_strategy="head-and-tail",
                notice="⚠️ [SHIELD]",
            ),
            prompts=[
                PromptRule(
                    pattern=re.compile(
                        r"Overwrite production schema\? \[y/N\]", re.I
                    ),
                    timeout=3.0,
                    verify_foreground=True,
                    action=lambda _p, stream: stream.write(b"y\n"),
                ),
            ],
            stream_engine="StatelessRegex",
        )

        # see: cans/05-storage-plane.md#Storage-plane
        .storage(
            idmap=IdmapConfig(guest_uid=0, host_uid=os.getuid()),
            shared_caches=[
                "~/.npm", "~/.cache/pip", "~/.cargo/registry",
            ],
            lock_broker=True,
            synthetic_files=[
                SyntheticFile.static(
                    "/workspace/.env.local",
                    "DATABASE_URL=postgres://app:vault@127.0.0.1:5432/prod\n",
                ),
                SyntheticFile.dynamic(
                    "/workspace/identity.jwt",
                    lambda _p: DynamicFile(
                        content=f"Bearer {uuid.uuid4().hex}",
                        mode=0o400,
                    ),
                ),
            ],
            mask_read=[
                MaskRule(
                    glob="**/secrets/**",
                    patterns=[
                        (r"sk_live_[0-9a-zA-Z]{24}", "[STRIPE_VAULT_KEY]"),
                        (r"AKIA[0-9A-Z]{16}", "[AWS_IAM_REDACTED]"),
                    ],
                ),
            ],
            snoops=[
                SnoopRule(
                    glob="~/.claude/sessions/*.json",
                    sync=True,
                    on_write=make_snoop(oracle),
                ),
            ],
        )

        # see: cans/07-wire-plane.md#Vault-and-credential-flow
        .wire(
            sinkhole=SinkholeConfig(
                allowlist=[
                    "api.stripe.com", "github.com", "*.internal.net",
                ],
                drop_all=True,
            ),
            ssh_agent_bridge=True,
            vaults=[
                CredentialDescriptor(
                    name="STRIPE_SECRET",
                    kind="bearer_token",
                    audience="api.stripe.com",
                    placeholder="vault://stripe_key",
                ),
            ],
            l4_proxies=[
                L4Proxy(
                    listen_port=5432,
                    destination="pg.internal.net:5432",
                    inject_mtls=Mtls(cert_path=pg_cert, key_path=pg_key),
                ),
            ],
            tap_filter="AllowUdpProxied",
        )

        # see: cans/plugins/chaos.md#Plugin-chaos
        .plugin(ChaosPlugin(ChaosConfig(
            network=[
                NetworkFaultRule(
                    target="api.stripe.com",
                    base_ms=150,
                    jitter_ms=30,
                    drop_rate=0.02,
                ),
                NetworkFaultRule(
                    target="api.stripe.com",
                    base_ms=150,
                    jitter_ms=30,
                    drop_rate=0.02,
                    direction="inbound",
                ),
                NetworkFaultRule(
                    target="api.stripe.com",
                    base_ms=150,
                    jitter_ms=30,
                    drop_rate=0.02,
                    direction="both",
                ),
            ],
            fs=[
                StorageFaultRule(
                    target_path="/workspace/build/cache/*",
                    simulate_error="ENOSPC",
                    error_probability=0.05,
                ),
            ],
        )))

        # see: cans/plugins/ledger.md#Merkle-WAL
        .plugin(LedgerPlugin(LedgerConfig(
            store="sqlite://.kelir/audit.merkle.db",
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
    )

    builder = builder.plugin(policy_plugin)

    # see: cans/02-plugin-model.md#Full-Verdict-lattice
    builder.trap("*", pre=make_universal_step(oracle), post=make_verify_receipt(oracle))
    builder.trap(TrapFilter.binary("npm").with_arg("test").with_arg("--").with_arg("--runInBand"), pre=make_warm_caches(oracle))
    builder.trap(TrapFilter.binary("git").with_arg("push"), pre=make_force_strip())
    builder.trap(TrapFilter.binary("git").with_arg("commit").with_arg("-m"), pre=make_commit_gate())
    builder.trap(TrapFilter.binary("git").with_arg("commit"), post=make_zero_byte_check())

    # see: cans/08-lifecycle.md#Watchdog
    @builder.on_vm_init
    async def _init(ctx: LifecycleCtx) -> None:
        ctx.set_watchdog(timeout=timedelta(minutes=5), action="reboot")

    # TODO: SSOT npm-ci via session.exec_in_guest dropped; live on_vm_ready delivers LifecycleCtx only, see cans/08-lifecycle.md#Session-and-context-handles
    @builder.on_vm_ready
    async def _ready(_ctx: LifecycleCtx | None) -> None:
        return None

    # TODO: SSOT ledger.flush on shutdown dropped; live on_vm_shutdown delivers LifecycleCtx only, see cans/08-lifecycle.md#Session-and-context-handles
    @builder.on_vm_shutdown
    async def _shutdown(_ctx: LifecycleCtx | None) -> None:
        return None

    # see: cans/08-lifecycle.md#Break-glass
    @builder.on_trap_deny
    async def _deny(event: TrapEvent, session: Session) -> None:
        if await session.cap.ledger.count_denials(event.ppid) > 5:
            await session.vm.suspend("./.kelir/quarantine.snap")
            await session.console()

    return builder


# see: cans/08-lifecycle.md#CoW-fork
async def fork_and_verify(
    parent: Session, hypothesis_id: str,
) -> tuple[str, SpawnResult, CausalDelta]:
    child = await parent.vm.fork()
    await child.vm.clock.skew(
        timedelta(days=24 * (ord(hypothesis_id) - ord("A") + 1)),
    )
    try:
        run = await child.spawn(
            "npm",
            ["run", f"verify-hypothesis-{hypothesis_id}"],
            cwd="/workspace/checkout-service",
        )
        delta = await child.get_causal_delta_from_parent()
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
        base = await session.vm.checkpoint("clean_base")
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
        winner = next(
            ((hid, d) for hid, run, d in branches if run.exit_code == 0),
            None,
        )
        if winner is None:
            await session.vm.rollback(base)
            return
        winning_id, winning_delta = winner

        # see: cans/08-lifecycle.md#Suspend-and-resume
        await session.suspend(External(ExternalSuspendKind.HUMAN_AUTHORIZATION_HOLD))
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
        final = await session.cap.ledger.finalize(
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
