// see: cans/02-plugin-model.md#Plugin-testkit
use std::path::PathBuf;

use kelir_interface::driver::{DriverPolicy, VmConfig, WxPolicy};

#[derive(Debug, Clone)]
pub struct SyntheticPtyStream {
    pub port: u32,
    pub chunks: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct BrokerUpperFile {
    pub path: PathBuf,
    pub contents: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SyntheticConfig {
    pub kernel_path: PathBuf,
    pub rootfs_path: PathBuf,
    pub memory_mb: u32,
    pub vcpus: u8,
}

pub fn synthetic_pty_stream(port: u32, chunks: Vec<Vec<u8>>) -> SyntheticPtyStream {
    SyntheticPtyStream { port, chunks }
}

pub fn synthetic_broker_upper(path: PathBuf, contents: Vec<u8>) -> BrokerUpperFile {
    BrokerUpperFile { path, contents }
}

pub fn synthetic_config() -> SyntheticConfig {
    SyntheticConfig {
        kernel_path: PathBuf::from("/boot/vmlinuz"),
        rootfs_path: PathBuf::from("/var/lib/kelir/rootfs.erofs"),
        memory_mb: 256,
        vcpus: 1,
    }
}

pub fn synthetic_vm_config() -> VmConfig {
    VmConfig {
        vcpus: 1,
        memory_mb: 256,
        kernel_path: PathBuf::from("/boot/vmlinuz"),
        rootfs_erofs_path: PathBuf::from("/var/lib/kelir/rootfs.erofs"),
        page_size_kb: 4,
        tsc_locked: true,
        entropy_seed: None,
        driver_policy: DriverPolicy {
            require_hardware_vmm: false,
            allow_emulated_fallback: true,
            verify_hw_lockfile: false,
            wx_enforcement: WxPolicy::Strict,
            ffi_worker_threads: 1,
            ffi_max_ops_per_sec: 0,
        },
        enable_gpu_passthrough: false,
        enable_watchdog: false,
        deterministic_clock: true,
        pinned_clocksource: None,
        ns_per_instruction: None,
        preemption_grace_ms: 0,
        network_off: true,
    }
}

// see: cans/15-operations.md#Multi-language-parity-gate
pub fn canonical_vector_names() -> Vec<&'static str> {
    // TODO (L3): serialize the synthetic fixtures themselves (TrapEvent JSON, raw frames, Decision expectations) instead of names only; the vectors.json wire format beyond {"vectors":[{"name"}]} is not pinned in cans/artifacts.
    vec![
        "synthetic_pty_stream",
        "synthetic_broker_upper",
        "synthetic_config",
        "synthetic_vm_config",
    ]
}
