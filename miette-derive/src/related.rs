use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    forward::WhichFn,
    utils::{display_pat_members, field_member, gen_all_variants_with},
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub struct Related(syn::Member);

impl Related {
    pub(crate) fn from_fields(fields: &syn::Fields) -> Option<Self> {
        for (i, field) in fields.iter().enumerate() {
            for attr in &field.attrs {
                if attr.path().is_ident("related") {
                    return Some(Related(field_member(i, field)));
                }
            }
        }
        None
    }

    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::Related,
            |ident, fields, DiagnosticConcreteArgs { related, .. }| {
                let (display_pat, _display_members) = display_pat_members(fields);
                related.as_ref().map(|related| {
                    let rel = match &related.0 {
                        syn::Member::Named(ident) => ident.clone(),
                        syn::Member::Unnamed(syn::Index { index, .. }) => {
                            format_ident!("_{}", index)
                        }
                    };
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
