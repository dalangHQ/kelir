// see: cans/01-architecture.md#Driver-backends
#![allow(non_camel_case_types)]
pub type hv_return_t = libc::c_int;
pub type hv_vcpu_t = u64;

pub const HV_SUCCESS: hv_return_t = 0;

// SAFETY: declarations only — Hypervisor.framework entry points per Apple docs; no call is made from safe code yet, and each call site must validate its vcpu handle before invoking.
// NOTE: crate-wide unsafe allow is forced by edition 2024 (`unsafe extern "C"` requires it); the only unsafe item in this crate is this extern block.
#[cfg_attr(target_os = "macos", link(name = "Hypervisor", kind = "framework"))]
unsafe extern "C" {
    pub fn hv_vm_create(flags: u64) -> hv_return_t;
    pub fn hv_vm_destroy() -> hv_return_t;
    pub fn hv_vcpu_create(
        vcpu: *mut hv_vcpu_t,
        exit: *mut *const libc::c_void,
        options: *const libc::c_void,
    ) -> hv_return_t;
    pub fn hv_vcpu_destroy(vcpu: hv_vcpu_t) -> hv_return_t;
    pub fn hv_vcpu_run(vcpu: hv_vcpu_t) -> hv_return_t;
    pub fn hv_vm_protect(ipa: u64, size: usize, flags: u64) -> hv_return_t;
}
