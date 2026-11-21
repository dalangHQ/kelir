// see: cans/02-plugin-model.md#Foreign-plugin-proxy
use super::*;

impl PipelineEngine {
    // TODO: [plugin_engine] fill in the L3 wave; pressure/keyring dispatch carries no path or device input, so no scope filter can evaluate here yet; slop: pressure dispatch skips scope filter entirely
    pub fn dispatch_host_pressure(&self, level: u32) -> Result<Verdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::HostPressure)?;
        for interceptor in &self.host_interceptors {
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_host_pressure(level),
                    || {
                        Ok(Verdict::Drop {
                            errno: Errno::Efault,
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
    }

    pub fn dispatch_host_keyring_access(&self, key: &str) -> Result<Verdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::HostKeyringAccess)?;
        for interceptor in &self.host_interceptors {
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_host_keyring_access(key),
                    || {
                        Ok(Verdict::Drop {
                            errno: Errno::Efault,
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            if !matches!(verdict, Verdict::Pass) {
                return Ok(verdict);
            }
        }
        drop(fused);
        Ok(Verdict::Pass)
    }

    // see: cans/11-protocol.md#Host-containment-verification
    pub fn dispatch_host_containment_breach(
        &self,
        pid: u32,
        path: &std::path::Path,
        kind: &str,
    ) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::HostContainmentBreach)?;
        for (index, interceptor) in self.host_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.host_scopes, index);
            if !scope.allows_path(path) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = {
                let result = quarantine_sync(
                    || interceptor.on_host_containment_breach(pid, path, kind),
                    || Err(PluginError::Halt("Plugin panicked".into())),
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                result
            };
            outcome?;
        }
        drop(fused);
        Ok(())
    }
}
