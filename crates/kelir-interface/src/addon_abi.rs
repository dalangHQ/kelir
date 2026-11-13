// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/03-sdk-architecture.md#Python-SDK
use std::error::Error;
use std::fmt;

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub const KELIR_ADDON_ABI_VERSION: u32 = 1;

// see: cans/03-sdk-architecture.md#TypeScript-SDK
#[derive(Debug)]
pub struct AddonAbiVersion(pub u32);

impl AddonAbiVersion {
    pub fn current() -> Self {
        Self(KELIR_ADDON_ABI_VERSION)
    }

    pub fn check(found: u32) -> Result<(), AddonLoadError> {
        if found == KELIR_ADDON_ABI_VERSION {
            Ok(())
        } else {
            Err(AddonLoadError::AbiMismatch {
                expected: KELIR_ADDON_ABI_VERSION,
                found,
            })
        }
    }
}

// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants
#[derive(Debug)]
pub enum AddonLoadError {
    MissingAddon,
    AbiMismatch { expected: u32, found: u32 },
}

impl fmt::Display for AddonLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAddon => write!(f, "kelir native addon is missing"),
            Self::AbiMismatch { expected, found } => write!(
                f,
                "kelir native addon ABI version mismatch: expected {expected}, found {found}"
            ),
        }
    }
}

impl Error for AddonLoadError {}
