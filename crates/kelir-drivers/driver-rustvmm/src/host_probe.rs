// see: cans/01-architecture.md#Driver-backends
// see: cans/15-operations.md#Developer-CLI
use std::path::Path;

// see: cans/01-architecture.md#Driver-backends
pub fn os_hypervisors(os: &str) -> &'static [&'static str] {
    match os {
        "linux" => &["kvm"],
        "macos" => &["hvf", "avf"],
        "windows" => &["whp"],
        "android" => &["avf"],
        _ => &[],
    }
}

// see: cans/15-operations.md#Developer-CLI
pub fn device_node(hypervisor: &str) -> Option<&'static str> {
    match hypervisor {
        "kvm" => Some("/dev/kvm"),
        "avf" => Some("/dev/avf"),
        // TODO (L3): entitlement and partition-capability nodes for the hvf and whp backends.
        _ => None,
    }
}

// see: cans/15-operations.md#Developer-CLI
pub fn device_present(hypervisor: &str) -> bool {
    device_node(hypervisor).is_some_and(|node| Path::new(node).exists())
}

// TODO (L3): gate every backend on a real capability probe; only kvm exposes a device node today.
pub fn available_hypervisors(os: &str) -> Vec<&'static str> {
    os_hypervisors(os)
        .iter()
        .copied()
        .filter(|hypervisor| *hypervisor != "kvm" || device_present("kvm"))
        .collect()
}
