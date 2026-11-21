// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
// see: cans/artifacts/code/session-accessors.rs#Plugin-deps-single-source-of-truth
use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn expand_plugin_deps(input: DeriveInput) -> TokenStream {
    let ident = &input.ident;
    // TODO: [plugin_engine] fill in the L3 wave; scan require::<T>() calls in register_interceptors and emit the Deps HList instead of NoDeps per cans/artifacts/code/session-accessors.rs#Plugin-deps-single-source-of-truth; slop: the derive always emits NoDeps, so interceptor deps are not enforced.
    let expanded = quote! {
        #[automatically_derived]
        impl kelir_interface::plugin_model::PluginDeps for #ident {
            type Deps = kelir_interface::plugin_model::NoDeps;
        }
    };
    TokenStream::from(expanded)
}
