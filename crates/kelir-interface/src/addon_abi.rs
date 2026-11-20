// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/03-sdk-architecture.md#Python-SDK

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub const KELIR_ADDON_ABI_VERSION: u32 = 1;

// see: cans/03-sdk-architecture.md#TypeScript-SDK
#[derive(Debug)]
pub struct AddonAbiVersion(pub u32);

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
#[derive(Debug)]
pub enum AddonLoadError {
    MissingAddon,
    AbiMismatch { expected: u32, found: u32 },
}
