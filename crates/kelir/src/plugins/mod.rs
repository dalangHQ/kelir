// Stub plugin crates for the examples. The real plugin crates have
// `new()` with different (or no) arguments and implement the SYNC
// interface traits, so the examples program against these standalone
// stub plugin structs, which implement the facade's async
// [`crate::plugin::KelirPlugin`] trait instead.
pub mod chaos;
pub mod ledger;
pub mod mesh;
pub mod replay;
pub mod supply_chain;
