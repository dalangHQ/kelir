# see: cans/08-lifecycle.md#Suspend-and-resume
# see: cans/08-lifecycle.md#Detached-lifecycle
from __future__ import annotations

from typing import Any

from kelir.generated.attest_telemetry import CausalDelta, SnapshotDescriptor
from kelir.sdk_errors_flow import EConfig


def _missing(what: str) -> EConfig:
    return EConfig(f"kelir native PyO3 bridge is missing: {what} (L3)")


# TODO (L3) [wire_plane]: deliver the ingress response over the parked vsock stream.; slop: respond always raises, parked ingress never answered.
async def ingress_respond(request: Any, response: Any) -> None:
    void = (request, response)
    raise _missing("ingress respond")


# TODO (L3) [lifecycle_checkpoint]: call native driver rollback via the PyO3 _kelir bridge.; slop: rollback no-ops, tainted state persists
async def session_rollback(vm: Any, snapshot: SnapshotDescriptor | None) -> None:
    void = (vm, snapshot)
    raise _missing("session rollback")


# TODO (L3) [bindings_py]: spawn binary in the guest via the PyO3 _kelir bridge.; slop: spawn returns Ok without guest process
async def session_spawn(
    binary: str, argv: list[str] | None, kwargs: dict[str, Any]
) -> Any:
    void = (binary, argv, kwargs)
    raise _missing("session spawn")


# TODO (L3) [vr_07_virtio_shm_unmap_on_teardown]: map the virtio_shm region shared across sibling microVMs.; slop: Session.shm always raises, no shared region exists.
async def session_shm(region: str) -> Any:
    void = region
    raise _missing("session shm")


# TODO (L3) [wire_plane]: splice the ingress listener directly to the guest shim via vsock.; slop: handle_ingress always raises, no live ingress path exists.
async def session_handle_ingress(
    port: int, request_type: Any, response_type: Any
) -> Any:
    void = (port, request_type, response_type)
    raise _missing("session handle_ingress")


# TODO (L3) [bindings_py]: exec binary in the guest via the PyO3 _kelir bridge.; slop: exec returns Ok without guest process
async def session_exec_in_guest(binary: str, argv: list[str]) -> Any:
    void = (binary, argv)
    raise _missing("session exec_in_guest")


# TODO (L3) [terminal_plane]: flush steering guidance onto the EXEC sub-lane 0x02 before the exit EOF frame once the native session exposes steer.; slop: steer raises, guidance never reaches the agent before exit.
async def session_steer(
    native: Any, exec_id: int, message: str
) -> None:
    void = (native, exec_id, message)
    raise _missing("session steer")


# TODO (L3) [terminal_plane]: open the PTY console master via the PyO3 _kelir bridge.; slop: PTY master never opened, console input lost
async def session_console() -> None:
    raise _missing("session console")


# TODO (L3) [lc_15_detached_session_reconnect_relay]: connect to relay.sock, verify the catalog lease is live, and rebuild the session from the runner-owned state.; slop: attach skips lease check, binds dead relay.sock
async def session_attach(session_id: str) -> Any:
    void = session_id
    raise _missing("TODO: Session.attach")
