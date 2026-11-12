# see: cans/02-plugin-model.md#Segregated-ring-traits
from __future__ import annotations

from kelir.generated.fusion_tables import HOOK_RINGS, LifecycleTag


# see: cans/02-plugin-model.md#Segregated-ring-traits
_VERB_RINGS: dict[str, tuple[str, LifecycleTag | None]] = {
    verb: (ring.value, lifecycle_tag)
    for verb, (ring, lifecycle_tag) in HOOK_RINGS.items()
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
