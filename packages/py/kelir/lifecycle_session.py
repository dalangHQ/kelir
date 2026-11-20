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
        return {"kind": "external", "detail": str(reason.kind)}
    return {"kind": "resource-pressure"}


# see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
class IngressStream:
    def __init__(self, request: Any) -> None:
        self.request = request

    async def respond(self, response: Any) -> None:
        # TODO (L3) [wire_plane]: deliver the ingress response over the parked vsock stream.; slop: respond always raises, parked ingress never answered.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: ingress respond (L3)")


class Session:
    def __init__(self) -> None:
        self._native: Any = None
        self._profile: Any | None = None
        self.driver_name: str = ""
        self.boot_ms: int = 0
        self._operator_lease_token: str | None = None
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
    def from_native(cls, handle: Any, profile: Any | None = None) -> Session:
        # L3-GAP: [bindings_py]: brand the native handle at the verified bridge boundary so only bridge-produced handles construct a Session.; slop: Session constructed from an unverified handle, boot check bypassed
        session = cls()
        session._native = handle
        session._profile = profile
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
        return Session.from_native(native.fork(), self._profile)

    # see: cans/03-sdk-architecture.md#Python-SDK
    # see: cans/plugins/profile.md#Bidirectional-extraction
    async def export_spec(self, path: str) -> None:
        if self._profile is None:
            raise EConfig(
                "Session.export_spec: session carries no ingested profile"
            )
        from kelir.lifecycle_sandbox import _profile_text

        # TODO (L2) [profile_ingestion]: overlay programmatic plane descriptors over the ingested profile before export.; slop: post-ingestion descriptor overrides silently absent from the export
        Path(path).write_text(
            _profile_text(self._profile, path), encoding="utf-8"
        )

    async def elevate_lease(self, token: str) -> None:
        if not isinstance(token, str) or not token:
            raise EConfig("session elevate_lease expects a non-empty lease token")
        # TODO (L3) [governance_tools]: verify the token signature against the ed25519 pubkey from the profile before accepting it.; slop: unverified token stored as the operator lease
        self._operator_lease_token = token

    # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    def cap_for(self, name: str) -> Any | None:
        return getattr(self.cap, name, None)

    # see: cans/03-sdk-architecture.md#Service-lookup-across-languages
    def plane(self, name: str) -> Any | None:
        return getattr(self, name, None)

    # see: cans/08-lifecycle.md#Clock-skew
    async def skew_clock(self, seconds: float) -> None:
        await self.vm.clock.skew(timedelta(seconds=seconds))

    async def rollback(self, snapshot: SnapshotDescriptor | None = None) -> None:
        if snapshot is not None:
            await self.vm.rollback(snapshot)
            return
        # L3-GAP: [lifecycle_checkpoint]: call native driver rollback via the PyO3 _kelir bridge.; slop: rollback no-ops, tainted state persists
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
        input: bytes | None = None,
        env: dict[str, str] | None = None,
    ) -> SpawnResult:
        # L3-GAP: [bindings_py]: spawn binary in the guest via the PyO3 _kelir bridge.; slop: spawn returns Ok without guest process
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session spawn (L3)")

    # see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    async def shm(self, region: str) -> Any:
        # TODO (L3) [vr_07_virtio_shm_unmap_on_teardown]: map the virtio_shm region shared across sibling microVMs.; slop: Session.shm always raises, no shared region exists.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session shm (L3)")

    # see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
    async def handle_ingress(
        self,
        *,
        port: int,
        request_type: Any = None,
        response_type: Any = None,
    ) -> Any:
        # TODO (L3) [wire_plane]: splice the ingress listener directly to the guest shim via vsock.; slop: handle_ingress always raises, no live ingress path exists.
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session handle_ingress (L3)")
        yield IngressStream(None)

    async def exec_in_guest(self, binary: str, argv: list[str]) -> SpawnResult:
        # L3-GAP: [bindings_py]: exec binary in the guest via the PyO3 _kelir bridge.; slop: exec returns Ok without guest process
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
        # L3-GAP: [terminal_plane]: open the PTY console master via the PyO3 _kelir bridge.; slop: PTY master never opened, console input lost
        raise EConfig("kelir native PyO3 bridge (_kelir) is missing: session console (L3)")

    # see: cans/08-lifecycle.md#Detached-lifecycle
    @classmethod
    async def attach(cls, session_id: str) -> Session:
        relay = kelir.relay_socket_path(session_id)
        _ = relay
        # L3-GAP: [lc_15_detached_session_reconnect_relay]: connect to relay.sock, verify the catalog lease is live, and rebuild the session from the runner-owned state.; slop: attach skips lease check, binds dead relay.sock
        raise EConfig("TODO: Session.attach (L3)")

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        return await self.cap.ledger.get_causal_delta_from_parent()
