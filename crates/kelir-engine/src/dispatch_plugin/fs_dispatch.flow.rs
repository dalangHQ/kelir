// see: cans/05-storage-plane.md#Interception-pipeline
use super::*;

impl PipelineEngine {
    pub fn dispatch_fs_write(
        &self,
        path: &std::path::Path,
        data: &mut [u8],
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<FilterAction, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::FsWrite)?;
        for (index, interceptor) in self.fs_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.fs_scopes, index);
            if !Self::fs_scope_allows(&scope, path, ctx) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let action = {
                let outcome = quarantine_sync(
                    || interceptor.on_fs_write(path, data, pid, ctx),
                    || {
                        Ok(FilterAction::Abort {
                            reason: "Plugin panicked".into(),
                        })
                    },
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            if !matches!(action, FilterAction::Forward) {
                return Ok(action);
            }
        }
        drop(fused);
        Ok(FilterAction::Forward)
    }

    pub fn dispatch_fs_lookup(
        &self,
        path: &std::path::Path,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::FsLookup)?;
        for (index, interceptor) in self.fs_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.fs_scopes, index);
            if !Self::fs_scope_allows(&scope, path, ctx) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_fs_lookup(path),
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

    pub fn dispatch_fs_read(
        &self,
        path: &std::path::Path,
        data: &[u8],
        ctx: &HandlerCtx,
    ) -> Result<Vec<u8>, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::FsRead)?;
        let mut current = data.to_vec();
        for (index, interceptor) in self.fs_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.fs_scopes, index);
            if !Self::fs_scope_allows(&scope, path, ctx) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let next = {
                let outcome: Result<Vec<u8>, PluginError> = quarantine_sync(
                    || interceptor.on_fs_read(path, &current),
                    || Err(PluginError::Halt("Plugin panicked".into())),
                );
                if fused.is_none() {
                    self.foreign_submit_complete();
                }
                outcome?
            };
            current = next;
        }
        drop(fused);
        Ok(current)
    }

    pub fn dispatch_fs_lock(
        &self,
        path: &std::path::Path,
        flock_cmd: i32,
        flock_type: i16,
        pid: u32,
        ctx: &HandlerCtx,
    ) -> Result<Verdict, PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::FsLock)?;
        for (index, interceptor) in self.fs_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.fs_scopes, index);
            if !Self::fs_scope_allows(&scope, path, ctx) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let verdict = {
                let outcome = quarantine_sync(
                    || interceptor.on_fs_lock(path, flock_cmd, flock_type, pid),
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
}
