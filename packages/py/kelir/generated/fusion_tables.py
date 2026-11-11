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
    EXEC_STDOUT = "exec-stdout"
    EXEC_STDERR = "exec-stderr"
    HEARTBEAT = "heartbeat"
    LIFECYCLE = "lifecycle"
    WATCHDOG = "watchdog"
    CUSTOM_ACTION = "custom-action"
    VSOCK_SERVICE = "vsock-service"
    FS_LOCK = "fs-lock"
    HOST_LSM_EXEC = "host-lsm-exec"
    HOST_PRESSURE = "host-pressure"
    HOST_KEYRING_ACCESS = "host-keyring-access"
    HOST_PREEMPT = "host-preempt"
    MEMORY_PRESSURE = "memory-pressure"


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
    "on_exec_stdout": (RingEvent.EXEC_STDOUT, None),
    "on_exec_stderr": (RingEvent.EXEC_STDERR, None),
    "on_heartbeat": (RingEvent.HEARTBEAT, None),
    "on_process_exit": (RingEvent.PROCESS_EXIT, None),
    "on_vm_init": (RingEvent.LIFECYCLE, "vm-init"),
    "on_vm_ready": (RingEvent.LIFECYCLE, "vm-ready"),
    "on_vm_shutdown": (RingEvent.LIFECYCLE, "vm-shutdown"),
    "on_watchdog": (RingEvent.WATCHDOG, None),
    "on_fs_lock": (RingEvent.FS_LOCK, None),
    "on_net_inbound": (RingEvent.NET_INBOUND, None),
    "on_host_lsm_exec": (RingEvent.HOST_LSM_EXEC, None),
    "on_host_pressure": (RingEvent.HOST_PRESSURE, None),
    "on_host_keyring_access": (RingEvent.HOST_KEYRING_ACCESS, None),
    "on_pre_commit": (RingEvent.LIFECYCLE, "pre-commit"),
    "on_post_commit": (RingEvent.LIFECYCLE, "post-commit"),
    "on_host_preempt": (RingEvent.HOST_PREEMPT, None),
    "on_memory_pressure": (RingEvent.MEMORY_PRESSURE, None),
}
