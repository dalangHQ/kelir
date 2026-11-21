// see: cans/plugins/db-proxy.md#Plugin-db-proxy
use kelir_interface::plugin_model::PluginError;

// see: cans/plugins/db-proxy.md#Service-exports-(db-proxy)
// TODO (L3) [relational_transaction]: parse the SQL statement through the libpgquery C-FFI AST classifier; slop: SQL passes unparsed, COPY PROGRAM and search_path evasion ride the regex path.
pub fn ast_parse_sql(sql: &str) -> Result<(), PluginError> {
    let _ = sql;
    Err(PluginError::Halt(
        "deferred to L3: libpgquery AST parse pending".into(),
    ))
}
