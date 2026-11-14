# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from datetime import timedelta
from pathlib import Path
from typing import Any

from kelir.generated.attest_telemetry import CausalDelta, SnapshotDescriptor
from kelir.sdk_errors import EConfig
import kelir

from kelir import (
    Caps,
    CheckpointHandle,
    External,
    HostHandle,
    ResourcePressure,
    SpawnResult,
    StorageHandle,
    SuspendReason,
    TerminalHandle,
    VmHandle,
    VsockHandle,
    WireHandle,
)


# see: cans/08-lifecycle.md#Suspend-and-resume
def _suspend_reason_record(reason: SuspendReason) -> dict[str, str]:
    if isinstance(reason, External):
        return {"kind": "external", "detail": reason.kind.value}
    return {"kind": "resource-pressure"}


class Session:
    def __init__(self) -> None:
        self._native: Any = None
        self.driver_name: str = ""
        self.boot_ms: int = 0
        self.cap = Caps()
        self.suspended_reason: SuspendReason | None = None
        self.vm = VmHandle(self)
        self.terminal = TerminalHandle()
        self.storage = StorageHandle()
        self.wire = WireHandle()
        self.checkpoint = CheckpointHandle()
        self.host = HostHandle()
        self.vsock = VsockHandle()

    # see: cans/03-sdk-architecture.md#Python-SDK
    @classmethod
    def from_native(cls, handle: Any) -> Session:
        # TODO (L3) [bindings_py]: brand the native handle at the verified bridge boundary so only bridge-produced handles construct a Session.; slop: Session constructed from an unverified handle, boot check bypassed
        session = cls()
        session._native = handle
        session.driver_name = str(handle.driver_name)
        session.boot_ms = int(handle.boot_ms)
        session.vm = VmHandle(session, handle.vm)
        session.terminal = TerminalHandle(handle.terminal)
        session.storage = StorageHandle(handle.storage)
        session.wire = WireHandle(handle.wire)
        session.checkpoint = CheckpointHandle(handle.checkpoint)
        session.host = HostHandle(handle.host)
        return session

    async def fork(self) -> Session:
        native = kelir._require_native(self._native, "session fork")
        return Session.from_native(native.fork())

    # see: cans/08-lifecycle.md#Clock-skew
    async def skew_clock(self, seconds: float) -> None:
        await self.vm.clock.skew(timedelta(seconds=seconds))

    async def rollback(self, snapshot: SnapshotDescriptor | None = None) -> None:
        if snapshot is not None:
            await self.vm.rollback(snapshot)
            return
        # TODO (L3) [lifecycle_checkpoint]: call native driver rollback via the PyO3 _kelir bridge.; slop: rollback no-ops, tainted state persists
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session rollback (L3)")

    async def stop(self) -> None:
        native = kelir._require_native(self._native, "session stop")
        native.stop()

    async def terminate(self) -> None:
        native = kelir._require_native(self._native, "session terminate")
        native.terminate()

    async def spawn(
        self,
        binary: str,
        argv: list[str] | None = None,
        *,
        cwd: str | None = None,
        params: Any = None,
    ) -> SpawnResult:
        # TODO (L3) [bindings_py]: spawn binary in the guest via the PyO3 _kelir bridge.; slop: spawn returns Ok without guest process
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session spawn (L3)")

    async def exec_in_guest(self, binary: str, argv: list[str]) -> SpawnResult:
        # TODO (L3) [bindings_py]: exec binary in the guest via the PyO3 _kelir bridge.; slop: exec returns Ok without guest process
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session exec_in_guest (L3)")

    async def suspend(self, reason: SuspendReason) -> None:
        if isinstance(reason, type) or not isinstance(
            reason, (External, ResourcePressure)
        ):
            raise EConfig(
                "session suspend reason must be External(kind) or "
                "ResourcePressure(), "
                f"got {type(reason).__name__}"
            )
        native = kelir._require_native(self._native, "session suspend")
        native.suspend(_suspend_reason_record(reason))
        self.suspended_reason = reason

    async def resume(self) -> None:
        native = kelir._require_native(self._native, "session resume")
        native.resume()
        self.suspended_reason = None

    # see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
    def session_dir(self) -> Path:
        native = kelir._require_native(self._native, "session session_dir")
        member = getattr(native, "session_dir", None)
        if member is None:
            raise EConfig(
                "kelir native PyO3 bridge (_kelir) is missing: session handle session_dir() (A11)"
            )
        return Path(str(member()))

    async def console(self) -> None:
        # TODO (L3) [terminal_plane]: open the PTY console master via the PyO3 _kelir bridge.; slop: PTY master never opened, console input lost
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session console (L3)")

    # see: cans/08-lifecycle.md#Detached-lifecycle
    @classmethod
    async def attach(cls, session_id: str) -> Session:
        relay = kelir.relay_socket_path(session_id)
        _ = relay
        # TODO (L3) [lc_15_detached_session_reconnect_relay]: connect to relay.sock, verify the catalog lease is live, and rebuild the session from the runner-owned state.; slop: attach skips lease check, binds dead relay.sock
        raise EConfig("TODO: Session.attach (L3)")

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        return await self.cap.ledger.get_causal_delta_from_parent()
