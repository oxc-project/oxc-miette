use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rustc_hash::FxHashSet;
use syn::spanned::Spanned;

use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    forward::WhichFn,
};

pub fn gen_all_variants_with(
    variants: &[DiagnosticDef],
    which_fn: WhichFn,
    mut f: impl FnMut(&syn::Ident, &syn::Fields, &DiagnosticConcreteArgs) -> Option<TokenStream>,
) -> Option<TokenStream> {
    let pairs = variants
        .iter()
        .filter_map(|def| {
            def.args.forward_or_override_enum(&def.ident, which_fn, |concrete| {
                f(&def.ident, &def.fields, concrete)
            })
        })
        .collect::<Vec<_>>();
    if pairs.is_empty() {
        return None;
    }
    let signature = which_fn.signature();
    let catchall = which_fn.catchall_arm();
    Some(quote! {
        #signature {
            #[allow(unused_variables, deprecated)]
            match self {
                #(#pairs)*
                #catchall
            }
        }
    })
}

use crate::fmt::Display;

pub fn field_member(index: usize, field: &syn::Field) -> syn::Member {
    match &field.ident {
        Some(ident) => syn::Member::Named(ident.clone()),
        None => syn::Member::Unnamed(syn::Index {
            index: u32::try_from(index).expect("field index exceeds u32::MAX"),
            span: field.span(),
        }),
    }
}

pub fn find_attr<'a>(fields: &'a syn::Fields, name: &str) -> Option<(usize, &'a syn::Field)> {
    fields
        .iter()
        .enumerate()
        .find(|(_, field)| field.attrs.iter().any(|attr| attr.path().is_ident(name)))
}

pub fn member_ident(member: &syn::Member) -> syn::Ident {
    match member {
        syn::Member::Named(ident) => ident.clone(),
        syn::Member::Unnamed(index) => format_ident!("_{}", index),
    }
}

pub fn gen_unused_pat(fields: &syn::Fields) -> TokenStream {
    match fields {
        syn::Fields::Named(_) => quote! { { .. } },
        syn::Fields::Unnamed(_) => quote! { ( .. ) },
        syn::Fields::Unit => quote! {},
    }
}

/// Goes in the slot `let Self #pat = self;` or `match self { Self #pat => ...
/// }`.
fn gen_fields_pat(fields: &syn::Fields) -> TokenStream {
    let member_idents = fields
        .iter()
        .enumerate()
        .map(|(i, field)| field.ident.clone().unwrap_or_else(|| format_ident!("_{}", i)));
    match fields {
        syn::Fields::Named(_) => quote! {
            { #(#member_idents),* }
        },
        syn::Fields::Unnamed(_) => quote! {
            ( #(#member_idents),* )
        },
        syn::Fields::Unit => quote! {},
    }
}

/// The returned tokens go in the slot `let Self #pat = self;` or `match self {
/// Self #pat => ... }`. The members can be passed to
/// `Display::expand_shorthand[_cloned]`.
pub fn display_pat_members(fields: &syn::Fields) -> (TokenStream, FxHashSet<syn::Member>) {
    let pat = gen_fields_pat(fields);
    let members = fields.iter().enumerate().map(|(i, field)| field_member(i, field)).collect();
    (pat, members)
}

impl Display {
    /// Returns `(fmt, args)` which must be passed to some kind of format macro
    /// without tokens in between, i.e. `format!(#fmt #args)`.
    pub(crate) fn expand_shorthand_cloned(
        &self,
        members: &FxHashSet<syn::Member>,
    ) -> (syn::LitStr, TokenStream) {
        let mut display = self.clone();
        display.expand_shorthand(members);
        let Display { fmt, args } = display;
        (fmt, args)
    }
}
