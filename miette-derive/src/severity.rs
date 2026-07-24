use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Token, parenthesized,
    parse::{Parse, ParseStream},
};

use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    forward::WhichFn,
    utils::gen_all_variants_with,
};

pub struct Severity(pub syn::Ident);

impl Parse for Severity {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        if ident == "severity" {
            let la = input.lookahead1();
            if la.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);
                let la = content.lookahead1();
                if la.peek(syn::LitStr) {
                    let str = content.parse::<syn::LitStr>()?;
                    let sev = get_severity(&str.value(), str.span())?;
                    Ok(Severity(syn::Ident::new(&sev, str.span())))
                } else {
                    let ident = content.parse::<syn::Ident>()?;
                    let sev = get_severity(&ident.to_string(), ident.span())?;
                    Ok(Severity(syn::Ident::new(&sev, ident.span())))
                }
            } else {
                input.parse::<Token![=]>()?;
                let str = input.parse::<syn::LitStr>()?;
                let sev = get_severity(&str.value(), str.span())?;
                Ok(Severity(syn::Ident::new(&sev, str.span())))
            }
        } else {
            Err(syn::Error::new(ident.span(), "MIETTE BUG: not a severity option"))
        }
    }
}

fn get_severity(input: &str, span: Span) -> syn::Result<String> {
    if input.eq_ignore_ascii_case("error") || input.eq_ignore_ascii_case("err") {
        Ok("Error".into())
    } else if input.eq_ignore_ascii_case("warning") || input.eq_ignore_ascii_case("warn") {
        Ok("Warning".into())
    } else if input.eq_ignore_ascii_case("advice")
        || input.eq_ignore_ascii_case("adv")
        || input.eq_ignore_ascii_case("info")
    {
        Ok("Advice".into())
    } else {
        Err(syn::Error::new(
            span,
            "Invalid severity level. Only Error, Warning, and Advice are supported.",
        ))
    }
}

impl Severity {
    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::Severity,
            |ident, fields, DiagnosticConcreteArgs { severity, .. }| {
                let severity = &severity.as_ref()?.0;
                let fields = match fields {
                    syn::Fields::Named(_) => quote! { { .. } },
                    syn::Fields::Unnamed(_) => quote! { (..) },
                    syn::Fields::Unit => quote! {},
                };
                Some(
                    quote! { Self::#ident #fields => std::option::Option::Some(miette::Severity::#severity), },
                )
            },
        )
    }

    pub(crate) fn gen_struct(&self) -> TokenStream {
        let sev = &self.0;
        quote! {
            fn severity(&self) -> std::option::Option<miette::Severity> {
                Some(miette::Severity::#sev)
            }
        }
    }
}
