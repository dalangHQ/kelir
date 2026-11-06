// see: cans/03-sdk-architecture.md#TypeScript-SDK
use napi_derive::napi;

#[napi]
pub struct NativeSessionHandle {
    // see: cans/03-sdk-architecture.md#TypeScript-SDK
}

#[napi]
pub struct NativeKelirBuilder {
    // see: cans/03-sdk-architecture.md#TypeScript-SDK
}

#[napi]
impl NativeKelirBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {}
    }

    #[napi]
    pub fn boot(&self) -> Result<NativeSessionHandle, napi::Error> {
        // see: cans/03-sdk-architecture.md#TypeScript-SDK
        Ok(NativeSessionHandle {})
    }
}

impl Default for NativeKelirBuilder {
    fn default() -> Self {
        Self::new()
    }
}
