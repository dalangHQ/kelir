// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
use napi::bindgen_prelude::Object;
use napi::{Error, Result, Status};
use napi_derive::napi;

pub mod fusion;

use fusion::{FusedTrampolineDescriptor, TrampolineTable, convert_descriptor};

pub struct PluginRegistration {
    pub name: String,
    pub enabled: bool,
}

use kelir_core::Session;
use kelir_interface::driver::{VmConfig, default_vm_config};

#[napi]
pub struct NativeSessionHandle {
    // TODO (L3): expose the engine plane handles over the NAPI session surface.
    #[allow(dead_code)]
    session: Option<Session>,
}

#[napi]
pub struct NativeKelirBuilder {
    config: VmConfig,
    plugins: Vec<PluginRegistration>,
    fused: Option<FusedTrampolineDescriptor>,
    trampolines: TrampolineTable,
}

#[napi]
impl NativeKelirBuilder {
    #[napi(constructor)]
    pub fn new(
        vcpus: Option<u8>,
        memory_mb: Option<u32>,
        kernel: Option<String>,
        rootfs_erofs: Option<String>,
        tsc_locked: Option<bool>,
        enable_watchdog: Option<bool>,
    ) -> Self {
        Self {
            config: default_vm_config(
                vcpus,
                memory_mb,
                kernel,
                rootfs_erofs,
                tsc_locked,
                enable_watchdog,
            ),
            plugins: Vec::new(),
            fused: None,
            trampolines: TrampolineTable::default(),
        }
    }

    #[napi]
    pub fn add_plugin(&mut self, name: String, enabled: Option<bool>) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::new(
                Status::InvalidArg,
                "add_plugin: plugin name must not be empty".to_string(),
            ));
        }
        if self.plugins.iter().any(|plugin| plugin.name == name) {
            return Ok(());
        }
        self.plugins.push(PluginRegistration {
            name,
            enabled: enabled.unwrap_or(true),
        });
        Ok(())
    }

    #[napi]
    pub fn fuse_hooks(&mut self, rings: Vec<Object>) -> Result<()> {
        if self.fused.is_some() {
            return Err(Error::from_reason(
                "fuse_hooks: the trampoline descriptor is frozen at seal".to_string(),
            ));
        }
        let descriptor = convert_descriptor(rings)?;
        self.trampolines =
            TrampolineTable::from_descriptor(&descriptor).map_err(Error::from_reason)?;
        self.fused = Some(descriptor);
        Ok(())
    }

    #[napi]
    pub fn trampoline_entry_count(&self) -> u32 {
        self.trampolines.entry_count()
    }

    #[napi]
    pub fn boot(&self) -> Result<NativeSessionHandle> {
        let Some(descriptor) = &self.fused else {
            return Err(Error::from_reason(
                "boot: fuse_hooks must run before boot".to_string(),
            ));
        };
        if descriptor.rings.len() as u32 != self.trampolines.entry_count() {
            return Err(Error::from_reason(
                "boot: the trampoline table is not one entry per ring event".to_string(),
            ));
        }
        for chain in &descriptor.rings {
            for hook in &chain.hooks {
                if let Some(plugin_name) = &hook.plugin_name {
                    match self
                        .plugins
                        .iter()
                        .find(|plugin| &plugin.name == plugin_name)
                    {
                        Some(registration) if registration.enabled => {}
                        Some(_) => {
                            return Err(Error::from_reason(format!(
                                "boot: fused ring {} carries hooks of disabled plugin `{plugin_name}`",
                                chain.ring.as_tag()
                            )));
                        }
                        None => {
                            return Err(Error::from_reason(format!(
                                "boot: fused ring {} carries hooks of unregistered plugin `{plugin_name}`",
                                chain.ring.as_tag()
                            )));
                        }
                    }
                }
            }
        }
        if let Some(reason) = self.config.missing_boot_path() {
            return Err(Error::new(Status::InvalidArg, reason.to_string()));
        }
        // TODO (L3): bind each fused ring chain to its single trampoline entry as one threadsafe closure and dispatch through the kelir-core pipeline.
        let plugins: Vec<kelir_core::PluginRegistration> = self
            .plugins
            .iter()
            .map(|plugin| kelir_core::PluginRegistration::new(plugin.name.clone(), plugin.enabled))
            .collect();
        let session = Session::boot_with_plugins(self.config.clone(), &plugins)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(NativeSessionHandle {
            session: Some(session),
        })
    }
}

impl Default for NativeKelirBuilder {
    fn default() -> Self {
        Self::new(None, None, None, None, None, None)
    }
}
