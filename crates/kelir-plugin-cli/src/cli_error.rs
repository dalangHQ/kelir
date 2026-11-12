// see: cans/15-operations.md#Developer-CLI
use std::error::Error;
use std::fmt;
use std::process::ExitCode;

use kelir_catalog::CatalogError;
use kelir_interface::driver_interface::VmmError;
use kelir_interface::plugin_model::PluginError;
use kelir_plugin_testkit::vectors_testkit::VectorError;

use crate::attest_cli::AttestError;
use crate::bump_core_cli::BumpCoreError;
use crate::fixtures_cli::FixtureError;
use crate::setup_cli::SetupError;
use crate::spec_cli::SpecError;

// see: cans/15-operations.md#Developer-CLI
#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Message(String),
    Io(std::io::Error),
    Attest(AttestError),
    BumpCore(BumpCoreError),
    Fixture(FixtureError),
    Setup(SetupError),
    Spec(SpecError),
    Vector(VectorError),
    Catalog(CatalogError),
    Plugin(PluginError),
    Vmm(VmmError),
}

impl CliError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self::Usage(message.into())
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage(_) => ExitCode::from(2),
            _ => ExitCode::FAILURE,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) | Self::Message(message) => write!(f, "{message}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::Attest(err) => write!(f, "{err}"),
            Self::BumpCore(err) => write!(f, "{err}"),
            Self::Fixture(err) => write!(f, "{err}"),
            Self::Setup(err) => write!(f, "{err}"),
            Self::Spec(err) => write!(f, "{err}"),
            Self::Vector(err) => write!(f, "{err}"),
            Self::Catalog(err) => write!(f, "{err}"),
            Self::Plugin(err) => write!(f, "{err}"),
            Self::Vmm(err) => write!(f, "{err}"),
        }
    }
}

impl Error for CliError {}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<AttestError> for CliError {
    fn from(err: AttestError) -> Self {
        Self::Attest(err)
    }
}

impl From<BumpCoreError> for CliError {
    fn from(err: BumpCoreError) -> Self {
        Self::BumpCore(err)
    }
}

impl From<FixtureError> for CliError {
    fn from(err: FixtureError) -> Self {
        Self::Fixture(err)
    }
}

impl From<SetupError> for CliError {
    fn from(err: SetupError) -> Self {
        Self::Setup(err)
    }
}

impl From<SpecError> for CliError {
    fn from(err: SpecError) -> Self {
        Self::Spec(err)
    }
}

impl From<VectorError> for CliError {
    fn from(err: VectorError) -> Self {
        Self::Vector(err)
    }
}

impl From<CatalogError> for CliError {
    fn from(err: CatalogError) -> Self {
        Self::Catalog(err)
    }
}

impl From<PluginError> for CliError {
    fn from(err: PluginError) -> Self {
        Self::Plugin(err)
    }
}

impl From<VmmError> for CliError {
    fn from(err: VmmError) -> Self {
        Self::Vmm(err)
    }
}
