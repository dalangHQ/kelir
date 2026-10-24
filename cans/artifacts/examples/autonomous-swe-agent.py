# examples/autonomous_swe_agent.py
"""
Full-surface Python SDK consumer for kelir 3.0.

Not a transliteration: this uses Pydantic for schema (the TS TypeBox
equivalent), async/await natively, decorator-registered handlers, and
`async with` for lifecycle enforcement. It exercises every plugin,
every VirtIO ring, and the three-tier evidence verification model.

Requirements:
    kelir>=3.0          (PyO3 bindings; ships plugin-pty, plugin-vfs,
                         plugin-wire, plugin-chaos, plugin-ledger)
    pydantic>=2.6
    blake3>=0.4
"""

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
from typing import Awaitable, Callable, Literal, Optional, Protocol, Self

from pydantic import BaseModel, Field

import kelir
from kelir import (
    CausalDelta,
    Decision,
    ExecutionReceipt,
    HandlerContext,
    KelirBuilder,
    Oracle,
    PostExecAction,
    Session,
    SnapshotDescriptor,
    SpawnResult,
    TrapAction,
    TrapEvent,
)
from kelir.plugin import BasePlugin, PluginError
from kelir.plugins.pty import (
    PromptRule,
    PtyPlugin,
    TokenShield,
)
from kelir.plugins.vfs import (
    DynamicFile,
    SyntheticFile,
    IdmapConfig,
    MaskRule,
    SnoopRule,
    VfsPlugin,
)
from kelir.plugins.wire import (
    L4Proxy,
    Mtls,
    SinkholeConfig,
    VaultEntry,
    WirePlugin,
)
from kelir.plugins.chaos import (
    ChaosConfig,
    FsChaos,
    NetworkChaos,
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


# ============================================================================
# 1. TYPED PARAMS — the Pydantic analogue of TypeBox's `.guard({ schema })`
# ============================================================================

class Env(str, Enum):
    STAGING = "staging"
    PRODUCTION = "production"


class FixStripeWebhook(BaseModel, frozen=True):
    """Validation target for every spawn in this file. If the runtime's
    `params` dict doesn't match, spawn fails with `EINVAL` *before* any
    hook fires — the TypeBox guarantee, enforced by pydantic."""
    action: Literal["fix-stripe-webhook"]
    repo: str
    issue_id: str = Field(alias="issueId")
    env: Env

    model_config = {"populate_by_name": True, "extra": "forbid"}


# ============================================================================
# 2. CUSTOM PLUGIN — BasePlugin across 3 of the 4 VirtIO rings
# ============================================================================

@dataclass
class RepoPolicyState:
    violations: int = 0
    current_branch: str = ""


@dataclass(frozen=True)
class RepoPolicyHandle:
    """Typed handle — the Python expression of §6's compile-time proof of
    installation. `plugin_with_handle` returns this object *only* if the
    plugin was successfully installed; removing the plugin from the chain
    means the handler closures have no `policy` to capture."""
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


# Taxonomy: urn:kelir:sys:trap:exec; rings: virtio-vsock, virtio-fs
class RepoPolicyPlugin(BasePlugin[RepoPolicyHandle]):
    name = "repo-policy"
    requires: tuple[str, ...] = ("ledger",)  # runtime dependency check

    def __init__(
        self,
        *,
        max_diff_bytes: int,
        protected_branches: list[str],
    ) -> None:
        self._opts = {
            "max_diff_bytes": max_diff_bytes,
            "protected_branches": tuple(protected_branches),
        }
        self._state = RepoPolicyState()

    def handle(self) -> RepoPolicyHandle:
        return RepoPolicyHandle(
            _state=self._state,
            _protected=self._opts["protected_branches"],
        )

    # ── Ring 2 (virtio-vsock): veto or rewrite `git push` ────────────────
    async def on_trap(self, event: TrapEvent) -> Optional[TrapAction]:
        is_git = event.binary_path.endswith("/git") or event.binary_path == "git"
        is_push = len(event.argv) > 1 and event.argv[1] == "push"
        if not (is_git and is_push):
            return None

        h = self.handle()
        if h.is_protected():
            h.record_violation()
            return TrapAction.deny(
                1, f"EPOLICY: '{h.current_branch}' is protected."
            )

        new_branch = f"agent/{uuid.uuid4().hex[:8]}"
        return TrapAction.rewrite(
            new_binary="/bin/git",
            new_argv=[*event.argv, f"HEAD:refs/heads/{new_branch}"],
            injected_env={"KELIR_REWRITTEN": "1"},
        )

    # ── Ring 3 (virtio-fs): reject oversize diff writes at FUSE boundary ─
    async def on_vfs_write(self, path: Path, data: bytes) -> bool:
        if path.suffix == ".diff" and len(data) > self._opts["max_diff_bytes"]:
            self.handle().record_violation()
            raise PluginError.reject(
                errno="ENOSPC", reason="diff exceeds policy ceiling"
            )
        return True

    # ── Ring 1 (virtio-console): annotate mandatory disclosures ─────────
    async def on_disclosure(self, body: bytes) -> Optional[bytes]:
        n = self.handle().violations
        return body + f"[repo-policy: {n} open violations]\n".encode()


# ============================================================================
# 3. SNOOP FACTORY — does NOT capture session; only HandlerContext (§12.4 T3)
# ============================================================================

def make_snoop(oracle: Oracle) -> Callable[[Path, bytes, HandlerContext], Awaitable[None]]:
    async def on_write(path: Path, raw: bytes, c: HandlerContext) -> None:
        evt = json.loads(raw)
        if evt.get("falsifiedResult") is True:
            raise PluginError.deny(f"EPERM: harness hallucination at {path}")
        producer_blake3 = await c.ledger.blake3_of_producing_process(path)
        await oracle.ingest_harness_event(path, evt, producer_blake3)
    return on_write


# ============================================================================
# 4. HANDLER FACTORIES — closure-captured typed handles (compile-time-proof
#    analogue for Python; missing handle → NameError at import time)
# ============================================================================

# `.before()` hooks gate the syscall and return a `Decision`; `.after()` hooks
# run post-execution and return a `PostExecAction`.
PreExecHandler = Callable[[HandlerContext], Awaitable[Decision]]
PostExecHandler = Callable[[HandlerContext], Awaitable[PostExecAction]]


def make_universal_step(oracle: Oracle) -> PreExecHandler:
    async def universal_step(c: HandlerContext) -> Decision:
        intent = await oracle.analyze_intent(c.trap)
        if not intent.is_hypothesis_valid:
            return Decision.deny(
                1,
                f"Verification Reject: {intent.falsification_reason or ''}",
            )
        if c.trap.entropy > 5.2 and not intent.allow_high_entropy:
            return Decision.deny(
                126, "EENTROPY: high-entropy argv requires oracle clearance."
            )
        return Decision.resume()
    return universal_step


def make_warm_caches(oracle: Oracle) -> PreExecHandler:
    async def warm_caches(c: HandlerContext) -> Decision:
        c.env.set("OTEL_TRACE_ID", oracle.current_trace_id())
        c.env.set("CI", "1")
        await c.fs.warm_cache(["node_modules", ".next/cache"])
        return Decision.resume()
    return warm_caches


def make_entropy_check() -> PreExecHandler:
    async def entropy_check(c: HandlerContext) -> Decision:
        if c.trap.entropy > 5.2:
            return Decision.deny(1, "EPERM: high-entropy payload.")
        if "--force" in c.trap.argv:
            return Decision.rewrite(
                binary="/bin/git",
                argv=[a for a in c.trap.argv if a != "--force"],
                env={"KELIR_DISARMED": "1"},
            )
        return Decision.resume()
    return entropy_check


def make_verify_receipt(oracle: Oracle) -> PostExecHandler:
    async def verify_receipt(c: HandlerContext) -> PostExecAction:
        pre: SnapshotDescriptor = c.step.last()  # raises if no checkpoint was marked
        receipt: ExecutionReceipt = await c.receipt()

        if receipt.binary_blake3 != c.trap.binary_blake3:
            await c.vm.rollback(pre)
            c.term.notice("⚡ [KELIR]: post-exec binary hash mismatch — REVERTED\n")
            return PostExecAction.rollback_and_halt()

        if (
            c.trap.binary_path.endswith("/npm")
            and receipt.egress_bytes > 0
            and "/checkout-service" in c.trap.cwd
        ):
            await c.vm.rollback(pre)
            c.term.notice(
                f"⚡ [KELIR]: hermetic test emitted {receipt.egress_bytes}B "
                "egress — REVERTED\n"
            )
            return PostExecAction.rollback_and_halt()

        if not await oracle.verify_physical_receipt(receipt):
            await c.vm.rollback(pre)
            c.term.notice("⚡ [KELIR]: post-condition violated — REVERTED\n")
            return PostExecAction.rollback_and_halt()

        return PostExecAction.proceed()
    return verify_receipt


def make_commit_gate(policy: RepoPolicyHandle) -> PreExecHandler:
    async def commit_gate(_c: HandlerContext) -> Decision:
        n = policy.violations
        if n > 0:
            return Decision.deny(1, f"EPOLICY: {n} prior violation(s).")
        return Decision.resume()
    return commit_gate


def make_zero_byte_check() -> PostExecHandler:
    async def zero_byte_check(c: HandlerContext) -> PostExecAction:
        receipt = await c.receipt()
        # dirty_bytes is always populated; zero is a real measurement
        if receipt.dirty_bytes == 0:
            raise RuntimeError("Verification anomaly: zero-byte delta commit attempted.")
        return PostExecAction.proceed()
    return zero_byte_check


# ============================================================================
# 5. RUNTIME BUILDER — the linear chain (write-only until `.boot()`)
# ============================================================================

async def build_runtime(
    *,
    oracle: Oracle,
    stripe_key: str,
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

        # ── TERMINAL PLANE (§7) ───────────────────────────────────────────
        .plugin(PtyPlugin(
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
        ))

        # ── STORAGE PLANE (§8) — in-flight read masking via MaskRule ──
        .plugin(VfsPlugin(
            idmap=IdmapConfig(guest_uid=0, host_uid=os.getuid()),
            shared_caches=[
                "~/.npm", "~/.cache/pip", "~/.cargo/registry",
            ],
            lock_broker=True,
            ghosts=[
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
                    on_write=make_snoop(oracle),  # does NOT capture session
                ),
            ],
        ))

        # ── WIRE PLANE (§9) ───────────────────────────────────────────────
        .plugin(WirePlugin(
            sinkhole=SinkholeConfig(
                allowlist=[
                    "api.stripe.com", "github.com", "*.internal.net",
                ],
                drop_all=True,
            ),
            ssh_agent_bridge=True,
            vaults=[
                VaultEntry(
                    name="STRIPE_SECRET",
                    placeholder="vault://stripe_key",
                    resolve=lambda: stripe_key,
                    scope_urls=["https://api.stripe.com/v1/*"],
                    scrub_response_headers=["set-cookie", "request-id"],
                ),
            ],
            l4_proxies=[
                L4Proxy(
                    listen_port=5432,
                    destination="pg.internal.net:5432",
                    inject_mtls=Mtls(cert_path=pg_cert, key_path=pg_key),
                ),
            ],
        ))

        # ── CHAOS (§10) ───────────────────────────────────────────────────
        .plugin(ChaosPlugin(ChaosConfig(
            network=[
                NetworkChaos(
                    target="api.stripe.com",
                    base_ms=150,
                    jitter_ms=30,
                    drop_rate=0.02,
                ),
            ],
            fs=[
                FsChaos(
                    target_path="/workspace/build/cache/*",
                    simulate_error="ENOSPC",
                    error_probability=0.05,
                ),
            ],
        )))

        # ── LEDGER (§12.1–§12.3) — must precede repo-policy (requires) ──
        .plugin(LedgerPlugin(LedgerConfig(
            # audit DB lives on the host, immune to guest rollback/fork
            store="sqlite://.kelir/audit.merkle.db",
            merkle_chain=True,
            otel=OtelConfig(
                endpoint="http://localhost:4318/v1/traces",
                service_name="kelir-swe-agent",
                propagate_traceparent=True,
            ),
            receipts=ReceiptConfig(
                include_syscall_count=True,
            ),
        )))
    )

    # ── CUSTOM PLUGIN + TYPED HANDLE ────────────────────────────────────
    #
    # `plugin_with_handle` returns `(self, handle)`. If the line is
    # removed, the handler closures below have no `policy` to capture
    # → NameError at import time. That is the Python expression
    # of §6's "downstream handlers fail to compile".
    builder, policy = builder.plugin_with_handle(policy_plugin)

    # ── EXEC GRAPH (§4.3, §12.5) ─────────────────────────────────────────
    def _register_handlers(cmd: kelir.ExecChain) -> None:
        # Universal atomic step loop: checkpoint → oracle → resume
        cmd.before("*", make_universal_step(oracle))
        cmd.before("npm test -- --runInBand", make_warm_caches(oracle))
        cmd.before("git push *", make_entropy_check())
        cmd.after("*", make_verify_receipt(oracle))
        cmd.before("git commit -m *", make_commit_gate(policy))
        cmd.after("git commit *", make_zero_byte_check())

    builder = builder.exec(_register_handlers)

    # ── LIFECYCLE HOOKS (§11.4, §12) — every handler receives `session` ─
    @builder.on_vm_init
    async def _init(driver: kelir.VmmDriver) -> None:
        await driver.set_watchdog(timeout=timedelta(minutes=5), action="reboot")

    @builder.on_vm_ready
    async def _ready(session: Session) -> None:
        await session.exec_in_guest("npm", ["ci", "--prefer-offline"])

    @builder.on_vm_shutdown
    async def _shutdown(session: Session) -> None:
        await session.ledger.flush()

    @builder.on_trap_deny
    async def _deny(event: TrapEvent, session: Session) -> None:
        if await session.ledger.count_denials(event.ppid) > 5:
            await session.vm.suspend("./.kelir/quarantine.snap")
            await session.break_glass()

    return builder


# ============================================================================
# 6. ORCHESTRATION — the seven phases
# ============================================================================

async def fork_and_verify(
    parent: Session, hypothesis_id: str,
) -> tuple[str, SpawnResult, CausalDelta]:
    child = await parent.vm.fork()                                   # <15ms
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
    stripe_key = os.environ["REAL_HOST_STRIPE_KEY"]

    builder = await build_runtime(
        oracle=oracle,
        stripe_key=stripe_key,
        pg_cert=Path("/etc/ssl/client.crt"),
        pg_key=Path("/etc/ssl/client.key"),
    )

    # `async with` consumes the builder, boots the microVM, and — on any
    # exception in the block — guarantees `session.terminate()` via
    # `__aexit__`. Rust gets this from `Drop`; Python gets it from the CM.
    async with builder as session:
        log.info("[kelir] booted driver=%s boot_ms=%s",
                 session.driver_name, session.boot_ms)

        await session.forward_port(8080, 3000)                       # §11.7
        base = await session.vm.checkpoint("clean_base")
        winning_delta: Optional[CausalDelta] = None

        # ── Phase 1: hermetic test (chaos on, egress forbidden) ────────
        test = await session.spawn(
            "npm",
            ["test", "--", "--runInBand"],
            cwd="/workspace/checkout-service",
            # params cross into the guest as the KELIR_PARAMS=<json>
            # environment variable
            params=FixStripeWebhook(
                action="fix-stripe-webhook",
                repo="checkout-service",
                issueId="ISSUE-8412",
                env=Env.STAGING,
            ),
        )
        if test.exit_code != 0:
            await session.vm.rollback(base)
            await session.break_glass()                              # §11.6
            return

        # ── Phase 2: 4-way CoW fork fan-out (§11.1, §11.3) ────────────
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

        # ── Phase 3: suspend to 0 MB RSS, await approval, resume (§11.2)
        await session.suspend("/tmp/eval.snap")
        log.info("[kelir] suspended rss_mb=0")
        await oracle.await_human_approval()
        await session.resume("/tmp/eval.snap")

        # ── Phase 4: overlay reconciliation + guest git commit (§8.1) ──
        stats = await session.fs.reconcile(
            cwd="/workspace/checkout-service",
            atomic_staging=True,
            fsync=True,
        )
        log.info("[kelir] reconciled files=%s deletes=%s",
                 stats.files_changed, stats.whiteouts_resolved)

        await session.spawn(
            "git",
            ["commit", "-am", f"fix(stripe): dedupe webhook body reads ({winning_id})"],
            cwd="/workspace/checkout-service",
        )

        # ── Phase 5: signed Merkle receipt (§12.3) ────────────────────
        final = await session.ledger.finalize(
            include_delta=winning_delta,
            include_merkle_root=True,
        )
        log.info("[kelir] receipt merkle=%s jws=%s",
                 final.merkle_root, final.signed_jws[:64])

        # `async with` handles the final suspend+terminate on exit.


# ============================================================================
# 7. ORACLE — user-supplied post-condition verification engine
# ============================================================================

class _IntentAnalysis(Protocol):
    is_hypothesis_valid: bool
    falsification_reason: Optional[str]
    allow_high_entropy: bool


class UserOracle(Oracle):
    async def analyze_intent(self, _trap: TrapEvent):
        return kelir.oracle.IntentAnalysis(
            is_hypothesis_valid=True,
            falsification_reason=None,
            allow_high_entropy=False,
        )

    async def verify_physical_receipt(self, _r: ExecutionReceipt) -> bool:
        return True

    async def ingest_harness_event(self, _p: Path, _e: dict, _b: str) -> None:
        return None

    async def await_human_approval(self) -> None:
        await asyncio.sleep(0.05)

    def current_trace_id(self) -> str:
        return uuid.uuid4().hex


# ============================================================================
# 8. ENTRY
# ============================================================================

if __name__ == "__main__":
    asyncio.run(main())
