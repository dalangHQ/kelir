// see: cans/03-sdk-architecture.md#TypeScript-SDK
use napi_derive::napi;

#[napi]
pub struct NativeSessionHandle {
    // TODO: Internal pointer to Arc<Session>
}

#[napi]
pub struct NativeKelirBuilder {
    // TODO: Accumulate plugin pointers
}

#[napi]
impl NativeKelirBuilder {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {}
    }

    #[napi]
    pub fn boot(&self) -> Result<NativeSessionHandle, napi::Error> {
        // TODO: Seal and boot microVM
        Ok(NativeSessionHandle {})
    }
}
