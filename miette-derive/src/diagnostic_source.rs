use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    forward::WhichFn,
    utils::{display_pat_members, field_member, gen_all_variants_with},
};
use proc_macro2::TokenStream;
use quote::quote;

pub struct DiagnosticSource(syn::Member);

impl DiagnosticSource {
    pub(crate) fn from_fields(fields: &syn::Fields) -> Option<Self> {
        for (i, field) in fields.iter().enumerate() {
            for attr in &field.attrs {
                if attr.path().is_ident("diagnostic_source") {
                    return Some(DiagnosticSource(field_member(i, field)));
                }
            }
        }
        None
    }

    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::DiagnosticSource,
            |ident, fields, DiagnosticConcreteArgs { diagnostic_source, .. }| {
                let (display_pat, _display_members) = display_pat_members(fields);
                diagnostic_source.as_ref().map(|diagnostic_source| {
                    let rel = match &diagnostic_source.0 {
                        syn::Member::Named(ident) => ident.clone(),
                        syn::Member::Unnamed(syn::Index { index, .. }) => {
                            quote::format_ident!("_{}", index)
                        }
                    };
                    quote! {
                        Self::#ident #display_pat => {
                            std::option::Option::Some(std::borrow::Borrow::borrow(#rel))
                        }
                    }
                })
            },
        )
    }

    pub(crate) fn gen_struct(&self) -> TokenStream {
        let rel = &self.0;
        quote! {
            fn diagnostic_source<'a>(&'a self) -> std::option::Option<&'a dyn miette::Diagnostic> {
                std::option::Option::Some(std::borrow::Borrow::borrow(&self.#rel))
            }
        }
    }
}
