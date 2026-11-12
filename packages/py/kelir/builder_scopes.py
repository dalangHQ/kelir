# see: cans/02-plugin-model.md#Segregated-ring-traits
from __future__ import annotations

from kelir.plugin_fusion import LifecycleTag


# see: cans/02-plugin-model.md#Segregated-ring-traits
_VERB_RINGS: dict[str, tuple[str, LifecycleTag | None]] = {
    "on_trap": ("trap", None),
    "on_process_exit": ("process-exit", None),
    "on_fs_lookup": ("fs-lookup", None),
    "on_fs_read": ("fs-read", None),
    "on_fs_write": ("fs-write", None),
    "on_fs_lock": ("fs-lock", None),
    "on_net_outbound": ("net-outbound", None),
    "on_net_inbound": ("net-inbound", None),
    "on_console_input": ("console-input", None),
    "on_console_output": ("console-output", None),
    "on_exec_stdout": ("exec-stdout", None),
    "on_exec_stderr": ("exec-stderr", None),
    "on_host_lsm_exec": ("host-lsm-exec", None),
    "on_host_pressure": ("host-pressure", None),
    "on_host_keyring_access": ("host-keyring-access", None),
    "on_vm_init": ("lifecycle", "vm-init"),
    "on_vm_ready": ("lifecycle", "vm-ready"),
    "on_vm_shutdown": ("lifecycle", "vm-shutdown"),
    "on_pre_commit": ("lifecycle", "pre-commit"),
    "on_post_commit": ("lifecycle", "post-commit"),
    "on_host_preempt": ("host-preempt", None),
    "on_memory_pressure": ("memory-pressure", None),
    "on_heartbeat": ("heartbeat", None),
    "on_watchdog": ("watchdog", None),
}


# see: cans/02-plugin-model.md#KelirPlugin-trait
_VERB_ARITY: dict[str, tuple[int, ...]] = {
    "on_trap": (1, 2),
    "on_process_exit": (1, 2),
    "on_fs_lookup": (1,),
    "on_fs_read": (2,),
    "on_fs_write": (4,),
    "on_fs_lock": (4,),
    "on_net_outbound": (1,),
    "on_net_inbound": (1,),
    "on_console_input": (2,),
    "on_console_output": (2,),
    "on_exec_stdout": (3,),
    "on_exec_stderr": (3,),
    "on_host_lsm_exec": (2,),
    "on_host_pressure": (1,),
    "on_host_keyring_access": (1,),
    "on_vm_init": (1,),
    "on_vm_ready": (1,),
    "on_vm_shutdown": (1,),
    "on_pre_commit": (1,),
    "on_post_commit": (2,),
    "on_host_preempt": (1,),
    "on_memory_pressure": (2,),
    "on_heartbeat": (1,),
    "on_watchdog": (1,),
}
