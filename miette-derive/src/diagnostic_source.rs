use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    forward::WhichFn,
    utils::{display_pat_members, field_member, find_attr, gen_all_variants_with, member_ident},
};
use proc_macro2::TokenStream;
use quote::quote;

pub struct DiagnosticSource(syn::Member);

impl DiagnosticSource {
    pub(crate) fn from_fields(fields: &syn::Fields) -> Option<Self> {
        let (index, field) = find_attr(fields, "diagnostic_source")?;
        Some(Self(field_member(index, field)))
    }

    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::DiagnosticSource,
            |ident, fields, DiagnosticConcreteArgs { diagnostic_source, .. }| {
                let (display_pat, _display_members) = display_pat_members(fields);
                diagnostic_source.as_ref().map(|diagnostic_source| {
                    let rel = member_ident(&diagnostic_source.0);
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
