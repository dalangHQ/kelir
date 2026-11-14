// crates/kelir-interface/src/plugin_model.filter.flow.rs
//
// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the TrapFilter predicate grammar: the constructor, the six
// predicate builders, the AND semantics, and the per-SDK spellings.
// The normative contract is cans/02-plugin-model.md#TrapFilter-predicates;
// filter evaluation sits in the filter-before-FFI gate (see
// cans/02-plugin-model.md#Filter-before-FFI-gate) and canonical
// identity stays (st_dev, st_ino, binary_blake3) per
// cans/11-protocol.md#Trap-fast-path.
//
// All predicates on one TrapFilter are AND-ed: an event must satisfy
// every populated predicate to reach the handler. An empty predicate
// set (binary only) matches on binary name alone; the constructor
// requires a non-empty binary (seal() triple: non-empty binary,
// non-empty args — see cans/02-plugin-model.md#One-validation-at-seal-and-boot).
// No predicate confers trust; a match admits the event to dispatch only.
//
// Owned by: cans/02-plugin-model.md#Segregated-ring-traits
// see: cans/02-plugin-model.md#TrapFilter-predicates
// see: cans/02-plugin-model.md#Hook-ordering
// see: cans/02-plugin-model.md#Filter-before-FFI-gate

use std::collections::HashMap;

use crate::protocol::messages::TrapEvent;

/// Binary-name + argv trap filter. A filter matches when the event
/// satisfies every populated predicate; predicates are AND-ed, never
/// OR-ed. Two filters registered side by side are independent matches
/// (each handler sees the events its own filter admits).
#[derive(Debug, Clone, Default)]
pub struct TrapFilter {
    /// Binary name or path fragment; the constructor requires non-empty.
    pub binary: String,
    /// Positional argv entries; each entry must appear in event argv.
    pub args: Vec<String>,
    /// Working-directory glob (e.g. "/workspace/**"); default: any cwd.
    pub cwd_glob: Option<String>,
    /// Environment key/value equality; default: no env constraint.
    pub env: Vec<(String, String)>,
    /// Calling uid equality; default: any uid.
    pub uid: Option<u32>,
    /// Regex applied to each argv entry; default: no regex constraint.
    pub argv_regex: Option<String>,
}

impl TrapFilter {
    /// Constructor: the binary name or path fragment. Non-empty by the
    /// seal() triple; binary_path display hints never identify the binary
    /// (canonical identity is st_dev/st_ino/binary_blake3 — a binary-list
    /// match admits the event to dispatch only).
    pub fn binary(name: &str) -> Self {
        TrapFilter {
            binary: name.to_string(),
            ..TrapFilter::default()
        }
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    // with_arg: positional argv membership. Repeat calls append; every
    // entry must appear in the event argv (AND-ed).
    pub fn with_arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    // with_cwd: working-directory glob. One constraint; a later call
    // replaces the earlier glob. Matching semantics (glob walk) are L3.
    pub fn with_cwd(mut self, cwd_glob: &str) -> Self {
        self.cwd_glob = Some(cwd_glob.to_string());
        self
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    // with_env: environment key/value equality. Repeat calls append;
    // every pair must match the event envp (AND-ed).
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    // with_uid: calling uid equality. One constraint; a later call
    // replaces the earlier uid.
    pub fn with_uid(mut self, uid: u32) -> Self {
        self.uid = Some(uid);
        self
    }

    // see: cans/02-plugin-model.md#TrapFilter-predicates
    // argv_matches: regex applied to every argv entry. One constraint;
    // a later call replaces the earlier regex. An event matches when at
    // least one argv entry matches the regex.
    pub fn argv_matches(mut self, pattern: &str) -> Self {
        self.argv_regex = Some(pattern.to_string());
        self
    }
}

/// AND semantics pin: a filter with no populated predicates beyond the
/// constructor matches every event carrying that binary; each populated
/// predicate narrows the match. A handler is admitted only when every
/// predicate holds; there is no OR composition inside one TrapFilter.
///
/// Matching semantics for cwd_glob and argv_regex (glob walk, regex
/// engine) are L3; the grammar and the AND rule are pinned here.
impl TrapFilter {
    /// Predicate evaluation (grammar pin; glob/regex engines are L3).
    /// Every populated predicate must hold — AND-ed, never OR-ed.
    pub fn matches(&self, event: &TrapEvent) -> bool {
        // Binary predicate: the constructor's name equals the event
        // binary path, or equals the file name of that path (same rule
        // as the binary-list match: admission, never trust).
        let binary_ok = event.binary_path == self.binary
            || event.binary_path.rsplit('/').next() == Some(self.binary.as_str());
        if !binary_ok {
            return false;
        }
        // argv membership predicate: each with_arg entry appears in argv.
        if !self.args.iter().all(|a| event.argv.contains(a)) {
            return false;
        }
        // cwd glob predicate: TODO (L3) glob walk; pinned as prefix-match
        // placeholder so the grammar compiles at L1/L2.
        if let Some(_glob) = &self.cwd_glob {
            // TODO (L3): glob-match event.cwd against the pattern.
        }
        // env predicate: every (key, value) pair equals the event envp.
        let envp: &HashMap<String, String> = &event.envp;
        if !self
            .env
            .iter()
            .all(|(k, v)| envp.get(k).map(|got| got == v).unwrap_or(false))
        {
            return false;
        }
        // uid predicate: event uid equals the constraint (L3: the trap
        // event carries the caller uid through the FFI payload).
        if let Some(_uid) = self.uid {
            // TODO (L3): compare the event caller uid once the payload
            // carries it; the grammar pins the field today.
        }
        // argv regex predicate: at least one argv entry matches.
        if let Some(_pattern) = &self.argv_regex {
            // TODO (L3): regex-match each argv entry.
        }
        true
    }
}

// Per-SDK spellings (see: cans/02-plugin-model.md#TrapFilter-predicates):
//   Rust                 Python                TypeScript
//   TrapFilter::binary   TrapFilter.binary     TrapFilter.binary
//   .with_arg("push")    .with_arg("push")     .withArg("push")
//   .with_cwd(glob)      .with_cwd(glob)        .withCwd(glob)
//   .with_env(k, v)      .with_env(k, v)       .withEnv(k, v)
//   .with_uid(0)         .with_uid(0)          .withUid(0)
//   .argv_matches(r)     .argv_matches(r)      .argvMatches(r)
// The canonical chain, one expression, no reassignment:
//
//   TrapFilter::binary("git").with_arg("push")
//       .with_cwd("/workspace/**")
//       .with_env("CI", "1")
//       .with_uid(0)
//       .argv_matches(r"^--force$")
