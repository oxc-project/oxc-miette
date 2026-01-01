use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Fields, Token, parenthesized,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    fmt::{self, Display},
    forward::WhichFn,
    utils::{display_pat_members, gen_all_variants_with},
};

pub enum Note {
    Display(Display),
    Field(syn::Member, Box<syn::Type>),
}

impl Parse for Note {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        if ident == "note" {
            let la = input.lookahead1();
            if la.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);
                let fmt = content.parse()?;
                let args = if content.is_empty() {
                    TokenStream::new()
                } else {
                    fmt::parse_token_expr(&content, false)?
                };
                let display = Display { fmt, args, has_bonus_display: false };
                Ok(Note::Display(display))
            } else {
                input.parse::<Token![=]>()?;
                Ok(Note::Display(Display {
                    fmt: input.parse()?,
                    args: TokenStream::new(),
                    has_bonus_display: false,
                }))
            }
        } else {
            Err(syn::Error::new(ident.span(), "not a note"))
        }
    }
}

impl Note {
    pub(crate) fn from_fields(fields: &syn::Fields) -> syn::Result<Option<Self>> {
        match fields {
            syn::Fields::Named(named) => Self::from_fields_vec(named.named.iter().collect()),
            syn::Fields::Unnamed(unnamed) => Self::from_fields_vec(unnamed.unnamed.iter().collect()),
            syn::Fields::Unit => Ok(None),
        }
    }

    fn from_fields_vec(fields: Vec<&syn::Field>) -> syn::Result<Option<Self>> {
        for (i, field) in fields.iter().enumerate() {
            for attr in &field.attrs {
                if attr.path().is_ident("note") {
                    let note = if let Some(ident) = field.ident.clone() {
                        syn::Member::Named(ident)
                    } else {
                        syn::Member::Unnamed(syn::Index { index: i as u32, span: field.span() })
                    };
                    return Ok(Some(Note::Field(note, Box::new(field.ty.clone()))));
                }
            }
        }
        Ok(None)
    }

    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::Note,
            |ident, fields, DiagnosticConcreteArgs { note, .. }| {
                let (display_pat, display_members) = display_pat_members(fields);
                match &note.as_ref()? {
                    Note::Display(display) => {
                        let (fmt, args) = display.expand_shorthand_cloned(&display_members);
                        Some(quote! {
                            Self::#ident #display_pat => std::option::Option::Some(std::boxed::Box::new(format!(#fmt #args))),
                        })
                    }
                    Note::Field(member, ty) => {
                        let note = match &member {
                            syn::Member::Named(ident) => ident.clone(),
                            syn::Member::Unnamed(syn::Index { index, .. }) => {
                                format_ident!("_{}", index)
                            }
                        };
                        let var = quote! { __miette_internal_var };
                        Some(quote! {
                            Self::#ident #display_pat => {
                                use miette::macro_helpers::ToOption;
                                miette::macro_helpers::OptionalWrapper::<#ty>::new().to_option(&#note).as_ref().map(|#var| -> std::boxed::Box<dyn std::fmt::Display + '_> { std::boxed::Box::new(format!("{}", #var)) })
                            },
                        })
                    }
                }
            },
        )
    }

    pub(crate) fn gen_struct(&self, fields: &Fields) -> Option<TokenStream> {
        let (display_pat, display_members) = display_pat_members(fields);
        match self {
            Note::Display(display) => {
                let (fmt, args) = display.expand_shorthand_cloned(&display_members);
                Some(quote! {
                    fn note(&self) -> std::option::Option<std::boxed::Box<dyn std::fmt::Display + '_>> {
                        #[allow(unused_variables, deprecated)]
                        let Self #display_pat = self;
                        std::option::Option::Some(std::boxed::Box::new(format!(#fmt #args)))
                    }
                })
            }
            Note::Field(member, ty) => {
                let var = quote! { __miette_internal_var };
                Some(quote! {
                    fn note(&self) -> std::option::Option<std::boxed::Box<dyn std::fmt::Display + '_>> {
                        #[allow(unused_variables, deprecated)]
                        let Self #display_pat = self;
                        use miette::macro_helpers::ToOption;
                        miette::macro_helpers::OptionalWrapper::<#ty>::new().to_option(&self.#member).as_ref().map(|#var| -> std::boxed::Box<dyn std::fmt::Display + '_> { std::boxed::Box::new(format!("{}", #var)) })
                    }
                })
            }
        }
    }
}
