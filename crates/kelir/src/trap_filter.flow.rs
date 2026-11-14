// see: cans/02-plugin-model.md#Filter-before-FFI-gate
use kelir_interface::fusion_plugin::FilterSnapshot;
use kelir_interface::protocol_wire::TrapEvent;

#[derive(Debug, Clone, Default)]
pub struct TrapFilter {
    pub binary: Option<String>,
    pub args: Vec<String>,
    pub pattern: Option<String>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub uid: Option<u32>,
    pub argv_matches: Option<regex::Regex>,
}

// see: cans/02-plugin-model.md#Chainable-verbs
impl From<&str> for TrapFilter {
    fn from(value: &str) -> Self {
        Self {
            binary: if value == "*" {
                None
            } else {
                Some(value.to_string())
            },
            args: Vec::new(),
            pattern: None,
            cwd: None,
            env: Vec::new(),
            uid: None,
            argv_matches: None,
        }
    }
}

impl TrapFilter {
    pub fn binary(binary: &str) -> Self {
        Self {
            binary: Some(binary.to_string()),
            args: Vec::new(),
            pattern: None,
            cwd: None,
            env: Vec::new(),
            uid: None,
            argv_matches: None,
        }
    }

    // see: cans/02-plugin-model.md#Filter-before-FFI-gate
    pub fn pattern(glob: &str) -> Self {
        Self {
            binary: None,
            args: Vec::new(),
            pattern: Some(glob.to_string()),
            cwd: None,
            env: Vec::new(),
            uid: None,
            argv_matches: None,
        }
    }

    pub fn with_arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    pub fn with_cwd(mut self, cwd: &str) -> Self {
        self.cwd = Some(cwd.to_string());
        self
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    pub fn with_uid(mut self, uid: u32) -> Self {
        self.uid = Some(uid);
        self
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    pub fn argv_matches(mut self, re: regex::Regex) -> Self {
        self.argv_matches = Some(re);
        self
    }
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
impl From<&TrapFilter> for FilterSnapshot {
    fn from(filter: &TrapFilter) -> Self {
        Self {
            binary: filter.binary.clone(),
            args: filter.args.clone(),
            pattern: filter.pattern.clone(),
            cwd: filter.cwd.clone(),
            env: filter.env.clone(),
            uid: filter.uid,
            argv: filter
                .argv_matches
                .as_ref()
                .map(|re| re.as_str().to_string()),
        }
    }
}

// see: cans/02-plugin-model.md#Filter-before-FFI-gate
// L3-GAP: [pe_06_filter_before_ffi_gate_latency]: pattern glob matching semantics per cans/02-plugin-model.md#Filter-before-FFI-gate.; slop: every event crosses FFI, scope checked in Python/JS
pub(crate) fn filter_matches(filter: &FilterSnapshot, event: &TrapEvent) -> bool {
    if filter
        .binary
        .as_ref()
        .is_some_and(|binary| binary != &event.binary_path)
    {
        return false;
    }
    filter.args.iter().all(|arg| event.argv.contains(arg))
}
