use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Fields, Token, parenthesized,
    parse::{Parse, ParseStream},
};

use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    fmt::{self, Display},
    forward::WhichFn,
    utils::{display_pat_members, field_member, find_attr, gen_all_variants_with, member_ident},
};

pub enum Help {
    Display(Display),
    Field(syn::Member, Box<syn::Type>),
}

impl Parse for Help {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        if ident == "help" {
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
                let display = Display { fmt, args };
                Ok(Help::Display(display))
            } else {
                input.parse::<Token![=]>()?;
                Ok(Help::Display(Display { fmt: input.parse()?, args: TokenStream::new() }))
            }
        } else {
            Err(syn::Error::new(ident.span(), "not a help"))
        }
    }
}

impl Help {
    pub(crate) fn from_fields(fields: &syn::Fields) -> Option<Self> {
        let (index, field) = find_attr(fields, "help")?;
        Some(Self::Field(field_member(index, field), Box::new(field.ty.clone())))
    }

    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::Help,
            |ident, fields, DiagnosticConcreteArgs { help, .. }| {
                let (display_pat, display_members) = display_pat_members(fields);
                match &help.as_ref()? {
                    Help::Display(display) => {
                        let (fmt, args) = display.expand_shorthand_cloned(&display_members);
                        Some(quote! {
                            Self::#ident #display_pat => std::option::Option::Some(std::borrow::Cow::Owned(format!(#fmt #args))),
                        })
                    }
                    Help::Field(member, ty) => {
                        let help = member_ident(member);
                        let var = quote! { __miette_internal_var };
                        Some(quote! {
                            Self::#ident #display_pat => {
                                use miette::macro_helpers::ToOption;
                                miette::macro_helpers::OptionalWrapper::<#ty>::new().to_option(&#help).as_ref().map(|#var| -> std::borrow::Cow<'_, str> { std::borrow::Cow::Owned(format!("{}", #var)) })
                            },
                        })
                    }
                }
            },
        )
    }

    pub(crate) fn gen_struct(&self, fields: &Fields) -> TokenStream {
        let (display_pat, display_members) = display_pat_members(fields);
        match self {
            Help::Display(display) => {
                let (fmt, args) = display.expand_shorthand_cloned(&display_members);
                quote! {
                    fn help(&self) -> std::option::Option<std::borrow::Cow<'_, str>> {
                        #[allow(unused_variables, deprecated)]
                        let Self #display_pat = self;
                        std::option::Option::Some(std::borrow::Cow::Owned(format!(#fmt #args)))
                    }
                }
            }
            Help::Field(member, ty) => {
                let var = quote! { __miette_internal_var };
                quote! {
                    fn help(&self) -> std::option::Option<std::borrow::Cow<'_, str>> {
                        #[allow(unused_variables, deprecated)]
                        let Self #display_pat = self;
                        use miette::macro_helpers::ToOption;
                        miette::macro_helpers::OptionalWrapper::<#ty>::new().to_option(&self.#member).as_ref().map(|#var| -> std::borrow::Cow<'_, str> { std::borrow::Cow::Owned(format!("{}", #var)) })
                    }
                }
            }
        }
    }
}
