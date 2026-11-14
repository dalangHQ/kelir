// crates/kelir-interface/src/wire_plane/sinkhole_rules.flow.rs
//
// ILLUSTRATIVE, NON-STANDALONE — do not compile this file directly.
// It pins the SinkholeConfig per-domain rule shape: the rules vector,
// the SinkholeRule fields, and the desugaring of the historical
// allowlist / drop_all shape into rules. The normative contract is
// cans/07-wire-plane.md#DNS-policy-filter; the wire plane owns the
// evaluation (allow/deny before upstream forward, NXDOMAIN on block).
//
// Owned by: cans/07-wire-plane.md#DNS-policy-filter
// see: cans/07-wire-plane.md#DNS-policy-filter
// see: cans/artifacts/code/vm-config-defaults.rs

/// Per-domain DNS admission rule. `host` carries exact names and
/// label-boundary wildcard suffixes ("*.internal.net"); matching
// mirrors the egress wildcard suffix rule (a suffix matches at a
/// label boundary; "evilexample.com" never matches "example.com").
/// `methods` and `paths` are HTTP-method/path constraints for L7-aware
/// rules; None means the rule applies regardless of method or path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkholeRule {
    /// Domain the rule governs: exact name or wildcard-suffix pattern.
    pub host: String,
    /// HTTP methods the rule admits (L7 rules); None = any method.
    pub methods: Option<Vec<String>>,
    /// HTTP paths the rule admits (L7 rules); None = any path.
    pub paths: Option<Vec<String>>,
    /// true = admit (allow rule); false = block (deny rule).
    pub allow: bool,
}

/// DNS sinkhole configuration: one ordered rule vector.
/// rules: first-match-wins over the ordered vector; a query with no
/// matching rule falls through to the default posture (see
/// cans/07-wire-plane.md#DNS-policy-filter: undeclared external targets
/// return synthetic NXDOMAIN).
#[derive(Debug, Clone, Default)]
pub struct SinkholeConfig {
    pub rules: Vec<SinkholeRule>,
}

impl SinkholeConfig {
    /// Sugar desugaring (pinned in cans/07-wire-plane.md#DNS-policy-filter):
    /// the historical allowlist / drop_all shape expands to rules.
    ///
    ///   SinkholeConfig { allowlist: vec![a, b], drop_all: false }
    ///     expands to rules = [
    ///       SinkholeRule { host: a, methods: None, paths: None, allow: true },
    ///       SinkholeRule { host: b, methods: None, paths: None, allow: true },
    ///     ]                                    // fall-through deny
    ///
    ///   SinkholeConfig { allowlist: vec![], drop_all: true }
    ///     expands to rules = []               // empty: everything falls through
    ///                                           // to the default posture
    ///   SinkholeConfig { allowlist: vec![a], drop_all: true }
    ///     expands to rules = [
    ///       SinkholeRule { host: a, methods: None, paths: None, allow: true },
    ///       SinkholeRule { host: "*", methods: None, paths: None, allow: false },
    ///     ]                                    // allow the named, deny the rest
    ///
    /// The sugar keeps old call sites compiling; the rule vector is
    /// the canonical storage. Field order in a rule literal is fixed
    /// (host, methods, paths, allow).
    pub fn from_allowlist(allowlist: Vec<String>, drop_all: bool) -> Self {
        let mut rules: Vec<SinkholeRule> = allowlist
            .into_iter()
            .map(|host| SinkholeRule {
                host,
                methods: None,
                paths: None,
                allow: true,
            })
            .collect();
        if drop_all && !rules.is_empty() {
            rules.push(SinkholeRule {
                host: "*".to_string(),
                methods: None,
                paths: None,
                allow: false,
            });
        }
        SinkholeConfig { rules }
    }
}

// Per-SDK spellings (see: cans/07-wire-plane.md#DNS-policy-filter):
//   Rust                  Python                     TypeScript
//   SinkholeConfig        SinkholeConfig             SinkholeConfig
//   { rules: Vec<…> }     rules=[SinkholeRule(...)]  { rules: [...] }
//   SinkholeRule {        SinkholeRule(              SinkholeRule {
//     host, methods,        host=…, methods=…,        host, methods,
//     paths, allow          paths=…, allow=…          paths, allow,
//   }                     )                          }
// The TS dnsExcept array is the same sugar spelled differently:
//   { dnsExcept: [names] } desugars to one allow rule per name, no
//   trailing deny (fall-through posture governs the rest).
