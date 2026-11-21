// see: cans/15-operations.md#Developer-CLI
use driver_rustvmm::host_probe;
use std::path::PathBuf;

use crate::cli_error::CliError;
use kelir_interface::driver_interface::{HW_LOCKFILE_PATH, verify_hw_lockfile};

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwLockPath(pub PathBuf);

impl HwLockPath {
    pub fn default_path() -> Self {
        Self(PathBuf::from(HW_LOCKFILE_PATH))
    }

    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProbe {
    Kvm,
    PageSize,
    Pidfd,
    Hvf,
    HvfEntitlement,
    Avf,
    Whp,
    KelirPack,
    SeccompInstall,
    WaitKillable,
    Userfaultfd,
}

impl HostProbe {
    pub fn name(&self) -> &'static str {
        match self {
            HostProbe::Kvm => "kvm",
            HostProbe::PageSize => "page-size",
            HostProbe::Pidfd => "pidfd",
            HostProbe::Hvf => "hvf",
            HostProbe::HvfEntitlement => "hvf-entitlement",
            HostProbe::Avf => "avf",
            HostProbe::Whp => "whp",
            HostProbe::KelirPack => "kelir-pack",
            HostProbe::SeccompInstall => "seccomp-install",
            HostProbe::WaitKillable => "wait-killable",
            HostProbe::Userfaultfd => "userfaultfd",
        }
    }
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeOutcome {
    Pass(String),
    Fail(String),
    NotApplicable(String),
    NotImplemented(String),
}

impl ProbeOutcome {
    pub fn is_ready(&self) -> bool {
        matches!(self, ProbeOutcome::Pass(_) | ProbeOutcome::NotApplicable(_))
    }

    pub fn status(&self) -> &'static str {
        match self {
            ProbeOutcome::Pass(_) => "pass",
            ProbeOutcome::Fail(_) => "fail",
            ProbeOutcome::NotApplicable(_) => "not-applicable",
            ProbeOutcome::NotImplemented(_) => "not-implemented",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            ProbeOutcome::Pass(detail)
            | ProbeOutcome::Fail(detail)
            | ProbeOutcome::NotApplicable(detail)
            | ProbeOutcome::NotImplemented(detail) => detail,
        }
    }
}

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub probes: Vec<(HostProbe, ProbeOutcome)>,
}

impl DoctorReport {
    pub fn is_ready(&self) -> bool {
        self.probes.iter().all(|(_, outcome)| outcome.is_ready())
    }

    pub fn pending(&self) -> Vec<String> {
        self.probes
            .iter()
            .filter_map(|(probe, outcome)| match outcome {
                ProbeOutcome::NotImplemented(reason) | ProbeOutcome::Fail(reason) => {
                    Some(format!("{}: {reason}", probe.name()))
                }
                ProbeOutcome::Pass(_) | ProbeOutcome::NotApplicable(_) => None,
            })
            .collect()
    }

    pub fn to_json(&self) -> String {
        let mut items = String::new();
        for (probe, outcome) in &self.probes {
            if !items.is_empty() {
                items.push(',');
            }
            items.push_str(&format!(
                "{{\"probe\":\"{}\",\"status\":\"{}\",\"detail\":\"{}\"}}",
                probe.name(),
                outcome.status(),
                outcome.detail()
            ));
        }
        format!("{{\"probes\":[{items}]}}")
    }
}

// see: cans/15-operations.md#Developer-CLI
fn probe_kvm() -> ProbeOutcome {
    if !host_probe::device_present("kvm") {
        return ProbeOutcome::Fail("/dev/kvm is absent".to_string());
    }
    // see: cans/15-operations.md#Developer-CLI
    match std::fs::metadata("/dev/kvm") {
        Ok(meta) => {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode() & 0o777;
            if mode == 0o660 {
                ProbeOutcome::Pass("/dev/kvm is present with mode 0660".to_string())
            } else {
                ProbeOutcome::Fail(format!("/dev/kvm has mode {mode:o}, expected 0660"))
            }
        }
        Err(err) => ProbeOutcome::Fail(format!("/dev/kvm stat failed: {err}")),
    }
}

fn probe_page_size() -> ProbeOutcome {
    if !cfg!(target_os = "linux") {
        // TODO: [hypervisor_driver]: read the host page size on non-Linux targets via the platform sysconf surface.; slop: Linux page size assumed, non-Linux probe never runs (deferred to L3)
        return ProbeOutcome::NotImplemented(
            "host page-size probe needs the platform sysconf".to_string(),
        );
    }
    match host_page_size_linux() {
        Ok(size) => ProbeOutcome::Pass(format!("host page size is {size} bytes")),
        Err(reason) => ProbeOutcome::Fail(reason),
    }
}

// see: cans/15-operations.md#Developer-CLI
fn host_page_size_linux() -> Result<u64, String> {
    const AT_PAGESZ: u64 = 6;
    let auxv =
        std::fs::read("/proc/self/auxv").map_err(|e| format!("/proc/self/auxv unreadable: {e}"))?;
    let entry_bytes = std::mem::size_of::<usize>() * 2;
    for entry in auxv.chunks_exact(entry_bytes) {
        let (a_type, a_val) = entry.split_at(entry_bytes / 2);
        let a_type = u64::from_le_bytes(
            a_type
                .try_into()
                .map_err(|_| "/proc/self/auxv entry is malformed".to_string())?,
        );
        if a_type == AT_PAGESZ {
            let a_val = u64::from_le_bytes(
                a_val
                    .try_into()
                    .map_err(|_| "/proc/self/auxv entry is malformed".to_string())?,
            );
            return Ok(a_val);
        }
    }
    Err("/proc/self/auxv carries no AT_PAGESZ entry".to_string())
}

fn probe_pidfd() -> ProbeOutcome {
    if !cfg!(target_os = "linux") {
        // TODO: [hypervisor_driver]: verify pidfd support on non-Linux targets.; slop: pidfd assumed present, non-Linux check never runs (deferred to L3)
        return ProbeOutcome::NotImplemented(
            "pidfd kernel probe needs a Linux pidfd_open syscall".to_string(),
        );
    }
    match pidfd_kernel_support_linux() {
        Ok((release, true)) => {
            ProbeOutcome::Pass(format!("pidfd_open needs kernel >= 5.3; running {release}"))
        }
        Ok((release, false)) => {
            ProbeOutcome::Fail(format!("pidfd_open needs kernel >= 5.3; running {release}"))
        }
        Err(reason) => ProbeOutcome::Fail(reason),
    }
}

// see: cans/15-operations.md#Developer-CLI
fn pidfd_kernel_support_linux() -> Result<(String, bool), String> {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map_err(|e| format!("/proc/sys/kernel/osrelease unreadable: {e}"))?;
    let trimmed = release.trim();
    let mut parts = trimmed.split('.');
    let major: u32 = parts
        .next()
        .unwrap_or_default()
        .parse()
        .map_err(|_| format!("kernel release `{trimmed}` is malformed"))?;
    let minor: u32 = parts
        .next()
        .unwrap_or_default()
        .parse()
        .map_err(|_| format!("kernel release `{trimmed}` is malformed"))?;
    Ok((trimmed.to_string(), (major, minor) >= (5, 3)))
}

fn probe_hvf() -> ProbeOutcome {
    // TODO: [hypervisor_driver]: probe HVF entitlements, Stage-2 unmap support, and PMCCNTR_EL0 access.; slop: HVF probe Ok, entitlements never checked (deferred to L3)
    ProbeOutcome::NotImplemented("HVF entitlement probe".to_string())
}

// see: cans/15-operations.md#Developer-CLI
fn probe_hvf_entitlement() -> ProbeOutcome {
    #[cfg(target_os = "macos")]
    {
        // TODO: [hypervisor_driver]: read com.apple.security.hypervisor via SecTaskCopyValueForEntitlement; slop: entitlement assumed, AMF denial surfaces at VM run (deferred to L3)
        ProbeOutcome::NotImplemented(
            "com.apple.security.hypervisor entitlement probe needs SecTask".to_string(),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        ProbeOutcome::NotApplicable(
            "com.apple.security.hypervisor is a macOS-only entitlement".to_string(),
        )
    }
}

// see: cans/15-operations.md#Developer-CLI
fn probe_kelir_pack() -> ProbeOutcome {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        if dir.join("kelir-pack").is_file() {
            return ProbeOutcome::Pass(format!(
                "kelir-pack found at {}",
                dir.join("kelir-pack").display()
            ));
        }
    }
    ProbeOutcome::Fail("kelir-pack not found on PATH (RAM EROFS compile needs it)".to_string())
}

// see: cans/15-operations.md#Developer-CLI
fn probe_avf() -> ProbeOutcome {
    if host_probe::device_present("avf") {
        ProbeOutcome::Pass("/dev/avf is present".to_string())
    } else {
        ProbeOutcome::Fail("/dev/avf is absent".to_string())
    }
}

fn probe_whp() -> ProbeOutcome {
    // TODO: [hypervisor_driver]: probe the Windows Hypervisor Platform partition capability.; slop: WHP probe Ok, partition capability never checked (deferred to L3)
    ProbeOutcome::NotImplemented("WHP partition probe".to_string())
}

// see: cans/15-operations.md#Developer-CLI
fn probe_seccomp_install() -> ProbeOutcome {
    // TODO: [hypervisor_driver]: install a live seccomp filter in a child and verify the kernel does not silently block or kill trapped syscalls (seccomp_install_probe).; slop: seccomp assumed installable, silent kernel block never detected (deferred to L3)
    // TODO: [hypervisor_driver]: run the seccomp_install_probe behavioral probe; slop: Class 2 pre-boot gating trusts the static /dev check while the kernel blocks installs (deferred to L3)
    ProbeOutcome::NotImplemented("seccomp install probe".to_string())
}

// see: cans/15-operations.md#Developer-CLI
fn probe_wait_killable() -> ProbeOutcome {
    // TODO: [hypervisor_driver]: verify a TASK_KILLABLE wait survives a trapped syscall without the kernel killing the waiter (wait_killable_probe).; slop: killable wait assumed, waiter killed silently at trap time (deferred to L3)
    // TODO: [hypervisor_driver]: run the wait_killable_probe behavioral probe; slop: Class 2 pre-boot gating trusts the static /dev check while the kernel kills killable waiters (deferred to L3)
    ProbeOutcome::NotImplemented("wait-killable probe".to_string())
}

// see: cans/15-operations.md#Developer-CLI
fn probe_userfaultfd() -> ProbeOutcome {
    // TODO: [hypervisor_driver]: verify userfaultfd registration succeeds for the guest memory range on this kernel.; slop: userfaultfd assumed available, CoW fault handling never verified (deferred to L3)
    // TODO: [hypervisor_driver]: run the userfaultfd behavioral probe; slop: Class 2 pre-boot gating trusts the static /dev check while userfaultfd registration fails (deferred to L3)
    ProbeOutcome::NotImplemented("userfaultfd probe".to_string())
}

// see: cans/15-operations.md#Developer-CLI
fn hypervisor_probe(hypervisor: &str) -> Option<(HostProbe, ProbeOutcome)> {
    match hypervisor {
        "kvm" => Some((HostProbe::Kvm, probe_kvm())),
        "hvf" => Some((HostProbe::Hvf, probe_hvf())),
        "avf" => Some((HostProbe::Avf, probe_avf())),
        "whp" => Some((HostProbe::Whp, probe_whp())),
        _ => None,
    }
}

// see: cans/01-architecture.md#Driver-backends
pub fn probe_host() -> DoctorReport {
    let mut probes = Vec::new();
    for hypervisor in host_probe::os_hypervisors(std::env::consts::OS) {
        if let Some(probe) = hypervisor_probe(hypervisor) {
            probes.push(probe);
        }
    }
    if cfg!(target_os = "macos") {
        probes.push((HostProbe::HvfEntitlement, probe_hvf_entitlement()));
    }
    // see: cans/15-operations.md#Developer-CLI
    probes.push((HostProbe::KelirPack, probe_kelir_pack()));
    if cfg!(target_os = "linux") {
        probes.push((HostProbe::PageSize, probe_page_size()));
        probes.push((HostProbe::Pidfd, probe_pidfd()));
        probes.push((HostProbe::SeccompInstall, probe_seccomp_install()));
        probes.push((HostProbe::WaitKillable, probe_wait_killable()));
        probes.push((HostProbe::Userfaultfd, probe_userfaultfd()));
    } else if cfg!(target_os = "macos") {
        probes.push((HostProbe::PageSize, probe_page_size()));
    } else if probes.is_empty() {
        probes.push((
            HostProbe::Kvm,
            ProbeOutcome::NotApplicable("no host probe for this OS".to_string()),
        ));
    }
    DoctorReport { probes }
}

// see: cans/15-operations.md#Developer-CLI
pub fn write_hw_lockfile(path: &HwLockPath, report: &DoctorReport) -> Result<(), CliError> {
    if let Some(parent) = path.as_path().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut contents = String::new();
    for (probe, outcome) in &report.probes {
        contents.push_str(&format!(
            "{}: {}: {}\n",
            probe.name(),
            outcome.status(),
            outcome.detail()
        ));
    }
    std::fs::write(path.as_path(), contents)?;
    // TODO: [hypervisor_driver]: set the immutable flag (chattr +i) on the lockfile.; slop: lockfile written world-writable, chattr +i never set (deferred to L3)
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), CliError> {
    let mut enforce = false;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--enforce" => enforce = true,
            "--json" => json = true,
            other => {
                return Err(CliError::usage(format!(
                    "doctor: unknown flag `{other}` (expected --enforce or --json)"
                )));
            }
        }
    }
    if enforce {
        let report = probe_host();
        if !report.is_ready() {
            return Err(format!(
                "doctor --enforce: host probes incomplete, refusing to write {}: {}",
                HwLockPath::default_path().as_path().display(),
                report.pending().join("; ")
            )
            .into());
        }
        let lock = HwLockPath::default_path();
        write_hw_lockfile(&lock, &report)?;
        // see: cans/15-operations.md#Developer-CLI
        verify_hw_lockfile()?;
        println!(
            "kelir doctor --enforce: wrote {} (verified via driver_interface::verify_hw_lockfile)",
            lock.as_path().display()
        );
        return Ok(());
    }
    let report = probe_host();
    if json {
        println!("{}", report.to_json());
    } else {
        for (probe, outcome) in &report.probes {
            println!(
                "doctor: {}: {}: {}",
                probe.name(),
                outcome.status(),
                outcome.detail()
            );
        }
    }
    if report.is_ready() {
        println!("kelir doctor: probe successful");
        Ok(())
    } else {
        Err(format!(
            "kelir doctor: host probes incomplete: {}",
            report.pending().join("; ")
        )
        .into())
    }
}
