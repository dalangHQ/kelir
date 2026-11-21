// see: cans/plugins/supply-chain.md#Plugin-supply-chain
use async_trait::async_trait;
use kelir_interface::plugin_model::{HandlerCtx, PluginError, TrapInterposer};
use kelir_interface::protocol_wire::{TrapEvent, TrappedSyscall, Verdict};

use crate::state::SupplyChainPlugin;

// see: cans/plugins/supply-chain.md#Invariants-and-policy-(supply-chain)
#[async_trait]
impl TrapInterposer for SupplyChainPlugin {
    async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> {
        // TODO (L3) [supply_chain_security]: mediate execve and memory syscalls through the W^X JIT gate; stub passes every trap.; slop: every trap Passes unmediated, the W^X JIT gate never enforces, execve and memory syscalls run unchecked.
        Err(PluginError::Halt(
            "TODO (L3): mediate execve and memory syscalls through the W^X JIT gate".into(),
        ))
    }
}

// TODO (L3) [supply_chain_security]: remove this allow when the stubbed on_trap lands and calls these mediators again.; slop: the allow hides dead mediators forever, the stubbed on_trap never lands, mediation stays unwired while the gate reports clean.
#[allow(dead_code)]
impl SupplyChainPlugin {
    fn mediate_execve(&self, _event: &TrapEvent) -> Result<Verdict, PluginError> {
        // TODO (L3) [supply_chain_security]: deny lifecycle scripts and mediate execve; stub passes.; slop: lifecycle scripts execute unmediated, execve passes unchecked, install-time code runs inside the guest.
        Err(PluginError::Halt(
            "TODO (L3): deny lifecycle scripts and mediate execve".into(),
        ))
    }

    fn mediate_memory_syscall(
        &self,
        _event: &TrapEvent,
        _syscall: &TrappedSyscall,
    ) -> Result<Verdict, PluginError> {
        // L3-GAP: [supply_chain_security]: the seccomp-unotify mask pins the trapped syscall set; payloads outside the mediated set pass; slop: mask pinned narrow, syscalls outside set pass unmediated
        // TODO (L3) [supply_chain_security]: mediate mprotect and memfd syscalls through the W^X gate; stub passes.; slop: mprotect and memfd syscalls pass unmediated, writable-executable mappings are created without a token check.
        Err(PluginError::Halt(
            "TODO (L3): mediate mprotect and memfd syscalls through the W^X gate".into(),
        ))
    }

    fn mediate_wx_gate(
        &self,
        _event: &TrapEvent,
        _operation: &str,
    ) -> Result<Verdict, PluginError> {
        // L3-GAP: [supply_chain_security]: measure the live calling task beyond the execve digest (ptrace or dynamic-link injection into a verified engine); slop: digest-only measurement, live task unmeasured, injected code undetected
        // TODO (L3) [supply_chain_security]: mint and deliver the W^X allocation token for authorized JIT engines; stub passes.; slop: the stub passes, so JIT engines allocate executable memory without any token ever being minted or verified.
        Err(PluginError::Halt(
            "TODO (L3): mint and deliver the W^X allocation token for authorized JIT engines"
                .into(),
        ))
    }

    fn blocks_lifecycle_script(&self, event: &TrapEvent) -> Result<bool, PluginError> {
        // L3-GAP: [sc_01_lifecycle_script_trap_exit126]: package-manager process-tree monitoring over the declared managers; dependency-directory lifecycle-script detection; nested npm→sh→node traps at the sh ancestor; slop: blocks_lifecycle_script stays Ok(false), lifecycle scripts Pass, exit never 126
        let _ = event;
        // TODO (L3) [sc_01_lifecycle_script_trap_exit126]: detect dependency-directory lifecycle scripts over the declared managers.; slop: blocks_lifecycle_script stays Ok(false), lifecycle scripts Pass, exit never 126.
        Ok(false)
    }

    fn is_bypass_runner(&self, _binary_path: &str) -> bool {
        // L3-GAP: [supply_chain_security]: bypass runner matching covers language toolchain layouts beyond file-name equality; slop: file-name matching only, toolchain layouts bypass the runner check
        // TODO (L3) [supply_chain_security]: match bypass runners by toolchain layout, not file name only; stub matches nothing.; slop: file-name matching only, toolchain-layout runners bypass the check, unverified interpreters run undetected.
        false
    }

    fn resolve_lockfile_dry_run(&self, _manifest: &str) -> Result<(), PluginError> {
        // TODO (L3) [supply_chain_security]: dry-run resolve the lockfile (npm/pip/pnpm/yarn/uv/poetry) and gate the resolved tree through OSV/Snyk/Socket/deps.dev advisories; slop: installs resolve live, a transitive CVE or malware package lands before any advisory lookup runs.
        Err(PluginError::Halt(
            "TODO (L3): dry-run lockfile resolution pending (L3)".into(),
        ))
    }

    fn vet_skill_file(&self, _path: &str) -> Result<(), PluginError> {
        // TODO (L3) [supply_chain_security]: vet SKILL.md and plugin files for unicode smuggling and prompt injection before guest ingestion; slop: unvetted skill files reach the guest, a smuggled instruction executes as trusted guidance.
        Err(PluginError::Halt(
            "TODO (L3): SKILL.md vetting pending (L3)".into(),
        ))
    }
}
