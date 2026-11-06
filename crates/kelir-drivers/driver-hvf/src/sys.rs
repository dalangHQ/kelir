// see: cans/01-architecture.md#Driver-backends
#![allow(non_camel_case_types)]
pub type hv_return_t = libc::c_int;
pub type hv_vcpu_t = u64;

pub const HV_SUCCESS: hv_return_t = 0;

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
