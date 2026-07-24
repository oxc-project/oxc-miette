use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    forward::WhichFn,
    utils::{display_pat_members, field_member, find_attr, gen_all_variants_with, member_ident},
};
use proc_macro2::TokenStream;
use quote::quote;

pub struct Related(syn::Member);

impl Related {
    pub(crate) fn from_fields(fields: &syn::Fields) -> Option<Self> {
        let (index, field) = find_attr(fields, "related")?;
        Some(Self(field_member(index, field)))
    }

    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::Related,
            |ident, fields, DiagnosticConcreteArgs { related, .. }| {
                let (display_pat, _display_members) = display_pat_members(fields);
                related.as_ref().map(|related| {
                    let rel = member_ident(&related.0);
                    quote! {
                        Self::#ident #display_pat => {
                            #rel.iter().map(|x| -> &(dyn miette::Diagnostic) { &*x }).collect()
                        }
                    }
                })
            },
        )
    }

    pub(crate) fn gen_struct(&self) -> TokenStream {
        let rel = &self.0;
        quote! {
            fn related(&self) -> miette::Related<'_> {
                use ::core::borrow::Borrow;
                self.#rel.iter().map(|x| -> &(dyn miette::Diagnostic) { &*x.borrow() }).collect()
            }
        }
    }
}
