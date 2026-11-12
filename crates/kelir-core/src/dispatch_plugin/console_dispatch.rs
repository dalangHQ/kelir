// see: cans/02-plugin-model.md#Unified-interposition-pipeline
use super::*;

impl PipelineEngine {
    pub async fn dispatch_console_input(
        &self,
        port: u32,
        bytes: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::ConsoleInput)?;
        for (index, interceptor) in self.console_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.console_scopes, index);
            if !scope.allows_device(VirtioDevice::Console) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(interceptor.on_console_input(port, bytes)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            *bytes = outcome?;
        }
        drop(fused);
        Ok(())
    }

    pub async fn dispatch_console_output(
        &self,
        port: u32,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::ConsoleOutput)?;
        for (index, interceptor) in self.console_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.console_scopes, index);
            if !scope.allows_device(VirtioDevice::Console) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(interceptor.on_console_output(port, chunk)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            *chunk = outcome?;
        }
        drop(fused);
        // see: cans/04-terminal-plane.md#Terminal-pipeline
        if let Some(terminal) = &self.terminal {
            terminal.record_output(LogStream::Port(port), chunk)?;
        }
        Ok(())
    }

    pub async fn dispatch_exec_stdout(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::ExecStdout)?;
        for (index, interceptor) in self.console_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.console_scopes, index);
            if !scope.allows_device(VirtioDevice::Console) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(interceptor.on_exec_stdout(pid, exec_id, chunk)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            *chunk = outcome?;
        }
        drop(fused);
        // see: cans/04-terminal-plane.md#Binary-exec-channel-(terminal-plane)
        if let Some(terminal) = &self.terminal {
            terminal.record_output(
                LogStream::Exec {
                    id: exec_id,
                    lane: EXEC_STREAM_STDOUT,
                },
                chunk,
            )?;
        }
        Ok(())
    }

    pub async fn dispatch_exec_stderr(
        &self,
        pid: u32,
        exec_id: u64,
        chunk: &mut Vec<u8>,
    ) -> Result<(), PluginError> {
        // see: cans/02-plugin-model.md#Foreign-plugin-proxy
        let fused = self.fused_crossing(TrampolineRing::ExecStderr)?;
        for (index, interceptor) in self.console_interceptors.iter().enumerate() {
            let scope = Self::scope_at(&self.console_scopes, index);
            if !scope.allows_device(VirtioDevice::Console) {
                continue;
            }
            if fused.is_none() {
                // see: cans/01-architecture.md#FFI-boundary-economics
                self.try_foreign_submit()?;
            }
            let outcome = quarantine_unit(interceptor.on_exec_stderr(pid, exec_id, chunk)).await;
            if fused.is_none() {
                self.foreign_submit_complete();
            }
            *chunk = outcome?;
        }
        drop(fused);
        // see: cans/04-terminal-plane.md#Binary-exec-channel-(terminal-plane)
        if let Some(terminal) = &self.terminal {
            terminal.record_output(
                LogStream::Exec {
                    id: exec_id,
                    lane: EXEC_STREAM_STDERR,
                },
                chunk,
            )?;
        }
        Ok(())
    }
}
