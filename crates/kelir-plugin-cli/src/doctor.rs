// see: cans/15-operations.md#Developer-CLI
use std::path::PathBuf;

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwLockPath(pub PathBuf);

impl HwLockPath {
    pub fn default_path() -> Self {
        Self(PathBuf::from("/etc/kelir/hw.lock"))
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
    Avf,
    Whp,
}

impl HostProbe {
    pub fn name(&self) -> &'static str {
        match self {
            HostProbe::Kvm => "kvm",
            HostProbe::PageSize => "page-size",
            HostProbe::Pidfd => "pidfd",
            HostProbe::Hvf => "hvf",
            HostProbe::Avf => "avf",
            HostProbe::Whp => "whp",
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

fn probe_kvm() -> ProbeOutcome {
    if std::path::Path::new("/dev/kvm").exists() {
        ProbeOutcome::Pass("/dev/kvm is present".to_string())
    } else {
        ProbeOutcome::Fail("/dev/kvm is absent".to_string())
    }
}

fn probe_page_size() -> ProbeOutcome {
    // TODO (L3): read the host page size via libc sysconf(_SC_PAGESIZE).
    ProbeOutcome::NotImplemented("host page-size probe needs libc sysconf (L3)".to_string())
}

fn probe_pidfd() -> ProbeOutcome {
    // TODO (L3): verify pidfd_open(2) and pidfd_send_signal(2) kernel support.
    ProbeOutcome::NotImplemented("pidfd kernel probe needs a pidfd_open syscall (L3)".to_string())
}

fn probe_hvf() -> ProbeOutcome {
    // TODO (L3): probe HVF entitlements, Stage-2 unmap support, and PMCCNTR_EL0 access.
    ProbeOutcome::NotImplemented("HVF entitlement probe (L3)".to_string())
}

fn probe_avf() -> ProbeOutcome {
    if std::path::Path::new("/dev/avf").exists() {
        ProbeOutcome::Pass("/dev/avf is present".to_string())
    } else {
        ProbeOutcome::Fail("/dev/avf is absent".to_string())
    }
}

fn probe_whp() -> ProbeOutcome {
    // TODO (L3): probe the Windows Hypervisor Platform partition capability.
    ProbeOutcome::NotImplemented("WHP partition probe (L3)".to_string())
}

pub fn probe_host() -> DoctorReport {
    let mut probes = Vec::new();
    if cfg!(target_os = "linux") {
        probes.push((HostProbe::Kvm, probe_kvm()));
        probes.push((HostProbe::PageSize, probe_page_size()));
        probes.push((HostProbe::Pidfd, probe_pidfd()));
    } else if cfg!(target_os = "macos") {
        probes.push((HostProbe::Hvf, probe_hvf()));
        probes.push((HostProbe::PageSize, probe_page_size()));
    } else if cfg!(target_os = "windows") {
        probes.push((HostProbe::Whp, probe_whp()));
    } else if cfg!(target_os = "android") {
        probes.push((HostProbe::Avf, probe_avf()));
    } else {
        probes.push((
            HostProbe::Kvm,
            ProbeOutcome::NotApplicable("no host probe for this OS".to_string()),
        ));
    }
    DoctorReport { probes }
}

// see: cans/15-operations.md#Developer-CLI
pub fn write_hw_lockfile(
    path: &HwLockPath,
    report: &DoctorReport,
) -> Result<(), Box<dyn std::error::Error>> {
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
    // TODO (L3): set the immutable flag (chattr +i) and attest the lockfile contents against live hardware.
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut enforce = false;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--enforce" => enforce = true,
            "--json" => json = true,
            other => {
                return Err(format!(
                    "doctor: unknown flag `{other}` (expected --enforce or --json)"
                )
                .into());
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
        println!("kelir doctor --enforce: wrote {}", lock.as_path().display());
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
