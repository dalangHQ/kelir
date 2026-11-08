# see: cans/02-plugin-model.md#Segregated-ring-traits
# see: cans/02-plugin-model.md#Hook-context-restriction
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
from __future__ import annotations

from enum import Enum
from typing import Literal


class RingEvent(str, Enum):
    TRAP = "trap"
    TRAP_DENY = "trap-deny"
    PROCESS_EXIT = "process-exit"
    FS_LOOKUP = "fs-lookup"
    FS_READ = "fs-read"
    FS_WRITE = "fs-write"
    NET_OUTBOUND = "net-outbound"
    NET_INBOUND = "net-inbound"
    CONSOLE_INPUT = "console-input"
    CONSOLE_OUTPUT = "console-output"
    HEARTBEAT = "heartbeat"
    LIFECYCLE = "lifecycle"
    WATCHDOG = "watchdog"
    CUSTOM_ACTION = "custom-action"
    VSOCK_SERVICE = "vsock-service"


class HookStage(str, Enum):
    PRE = "pre"
    POST = "post"
    SINGLE = "single"


LifecycleTag = Literal[
    "vm-init",
    "vm-ready",
    "vm-shutdown",
    "trap-deny",
    "pre-commit",
    "post-commit",
]

LIFECYCLE_TAGS: tuple[str, ...] = (
    "vm-init",
    "vm-ready",
    "vm-shutdown",
    "trap-deny",
    "pre-commit",
    "post-commit",
)

HOOK_RINGS: dict[str, tuple[RingEvent, LifecycleTag | None]] = {
    "on_trap": (RingEvent.TRAP, None),
    "on_fs_lookup": (RingEvent.FS_LOOKUP, None),
    "on_fs_read": (RingEvent.FS_READ, None),
    "on_fs_write": (RingEvent.FS_WRITE, None),
    "on_net_outbound": (RingEvent.NET_OUTBOUND, None),
    "on_console_input": (RingEvent.CONSOLE_INPUT, None),
    "on_console_output": (RingEvent.CONSOLE_OUTPUT, None),
    "on_heartbeat": (RingEvent.HEARTBEAT, None),
    "on_process_exit": (RingEvent.PROCESS_EXIT, None),
    "on_vm_init": (RingEvent.LIFECYCLE, "vm-init"),
    "on_vm_ready": (RingEvent.LIFECYCLE, "vm-ready"),
    "on_vm_shutdown": (RingEvent.LIFECYCLE, "vm-shutdown"),
    "on_watchdog": (RingEvent.WATCHDOG, None),
}
