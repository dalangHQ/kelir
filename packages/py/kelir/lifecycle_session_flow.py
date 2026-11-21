# see: cans/03-sdk-architecture.md#Python-SDK
from __future__ import annotations

from datetime import timedelta
from pathlib import Path
from typing import Any

from kelir.generated.attest_telemetry import CausalDelta, SnapshotDescriptor
from kelir.sdk_errors_flow import EConfig
import kelir
from kelir.lifecycle_session_run import (
    ingress_respond as _run_ingress_respond,
    session_attach as _run_session_attach,
    session_console as _run_session_console,
    session_exec_in_guest as _run_session_exec_in_guest,
    session_handle_ingress as _run_session_handle_ingress,
    session_rollback as _run_session_rollback,
    session_shm as _run_session_shm,
    session_spawn as _run_session_spawn,
    session_steer as _run_session_steer,
)

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
        await _run_ingress_respond(self.request, response)


# see: cans/02-plugin-model.md#Unified-verdict
class ExecHandle:
    def __init__(self, session: Session, exec_id: int) -> None:
        self._session = session
        self.exec_id = exec_id

    async def steer(self, message: str) -> None:
        await self._session.steer(self.exec_id, message)


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
        # TODO: [bindings_py]: brand the native handle at the verified bridge boundary so only bridge-produced handles construct a Session.; slop: Session constructed from an unverified handle, boot check bypassed
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
        from kelir.lifecycle_sandbox_run import _profile_text, _write_profile_text

        # TODO (L2) [profile_ingestion]: overlay programmatic plane descriptors over the ingested profile before export.; slop: post-ingestion descriptor overrides silently absent from the export
        _write_profile_text(path, _profile_text(self._profile, path))

    async def elevate_lease(self, token: str) -> None:
        if not isinstance(token, str) or not token:
            raise EConfig("session elevate_lease expects a non-empty lease token")
        # see: cans/03-sdk-architecture.md#Python-SDK
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
        await _run_session_rollback(self.vm, snapshot)

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
        return await _run_session_spawn(
            binary, argv, dict(cwd=cwd, params=params, input=input, env=env)
        )

    # see: cans/plugins/mesh.md#CoW-swarm-mesh-federation
    async def shm(self, region: str) -> Any:
        return await _run_session_shm(region)

    # see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
    async def handle_ingress(
        self,
        *,
        port: int,
        request_type: Any = None,
        response_type: Any = None,
    ) -> Any:
        return await _run_session_handle_ingress(
            port, request_type, response_type
        )

    async def exec_in_guest(self, binary: str, argv: list[str]) -> SpawnResult:
        return await _run_session_exec_in_guest(binary, argv)

    # see: cans/02-plugin-model.md#Unified-verdict
    async def steer(self, exec_id: int, message: str) -> None:
        if isinstance(exec_id, bool) or not isinstance(exec_id, int):
            raise EConfig("session steer expects an int exec_id")
        if not isinstance(message, str) or not message:
            raise EConfig("session steer expects a non-empty guidance message")
        native = kelir._require_native(self._native, "session steer")
        steer = getattr(native, "steer", None)
        if steer is None:
            await _run_session_steer(native, exec_id, message)
            return
        steer(exec_id, message)

    def exec_handle(self, exec_id: int) -> ExecHandle:
        return ExecHandle(self, exec_id)

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
        await _run_session_console()

    # see: cans/08-lifecycle.md#Detached-lifecycle
    @classmethod
    async def attach(cls, session_id: str) -> Session:
        return Session.from_native(await _run_session_attach(session_id))

    async def get_causal_delta_from_parent(self) -> CausalDelta:
        return await self.cap.ledger.get_causal_delta_from_parent()
