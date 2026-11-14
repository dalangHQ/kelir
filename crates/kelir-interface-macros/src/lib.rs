// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
// see: cans/artifacts/code/session-accessors.rs#Plugin-deps-single-source-of-truth
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

// see: cans/artifacts/code/session-accessors.rs#Plugin-deps-single-source-of-truth
#[proc_macro_derive(PluginDeps)]
pub fn derive_plugin_deps(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = &input.ident;
    // TODO (L3): scan require::<T>() calls in register_interceptors and emit the Deps HList instead of NoDeps per cans/artifacts/code/session-accessors.rs#Plugin-deps-single-source-of-truth.
    let expanded = quote! {
        #[automatically_derived]
        impl kelir_interface::plugin_model::PluginDeps for #ident {
            type Deps = kelir_interface::plugin_model::NoDeps;
        }
    };
    TokenStream::from(expanded)
}
