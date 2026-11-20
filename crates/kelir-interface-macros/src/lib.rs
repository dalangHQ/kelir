// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
// see: cans/artifacts/code/session-accessors.rs#Plugin-deps-single-source-of-truth
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

#[path = "lib.flow.rs"]
mod lib_flow;

// see: cans/artifacts/code/session-accessors.rs#Plugin-deps-single-source-of-truth
#[proc_macro_derive(PluginDeps)]
pub fn derive_plugin_deps(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    lib_flow::expand_plugin_deps(input)
}
