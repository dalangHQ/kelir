// see: cans/03-sdk-architecture.md#TypeScript-SDK
use std::error::Error;
use std::fmt;

use crate::addon_abi::{AddonAbiVersion, AddonLoadError, KELIR_ADDON_ABI_VERSION};

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
