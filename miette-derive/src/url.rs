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
    utils::{display_pat_members, gen_all_variants_with, gen_unused_pat},
};

pub enum Url {
    Display(Display),
    DocsRs,
}

impl Parse for Url {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        if ident == "url" {
            let la = input.lookahead1();
            if la.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);
                if content.peek(syn::LitStr) {
                    let fmt = content.parse()?;
                    let args = if content.is_empty() {
                        TokenStream::new()
                    } else {
                        fmt::parse_token_expr(&content, false)?
                    };
                    let display = Display { fmt, args };
                    Ok(Url::Display(display))
                } else {
                    let option = content.parse::<syn::Ident>()?;
                    if option == "docsrs" {
                        Ok(Url::DocsRs)
                    } else {
                        Err(syn::Error::new(
                            option.span(),
                            "Invalid argument to url() sub-attribute. It must be either a string or a plain `docsrs` identifier",
                        ))
                    }
                }
            } else {
                input.parse::<Token![=]>()?;
                Ok(Url::Display(Display { fmt: input.parse()?, args: TokenStream::new() }))
            }
        } else {
            Err(syn::Error::new(ident.span(), "not a url"))
        }
    }
}

impl Url {
    #[expect(
        clippy::literal_string_with_formatting_args,
        reason = "this string becomes the format template emitted by the derive macro"
    )]
    fn expand(&self, fields: &Fields, item_path: &str) -> (TokenStream, String, TokenStream) {
        match self {
            Url::Display(display) => {
                let (pat, members) = display_pat_members(fields);
                let (fmt, args) = display.expand_shorthand_cloned(&members);
                (pat, fmt.value(), args)
            }
            Url::DocsRs => (
                gen_unused_pat(fields),
                "https://docs.rs/{crate_name}/{crate_version}/{mod_name}/{item_path}".into(),
                quote! {
                    ,
                    crate_name=env!("CARGO_PKG_NAME"),
                    crate_version=env!("CARGO_PKG_VERSION"),
                    mod_name=env!("CARGO_PKG_NAME").replace('-', "_"),
                    item_path=#item_path
                },
            ),
        }
    }

    pub(crate) fn gen_enum(
        enum_name: &syn::Ident,
        variants: &[DiagnosticDef],
    ) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::Url,
            |ident, fields, DiagnosticConcreteArgs { url, .. }| {
                let item_path = format!("enum.{enum_name}.html#variant.{ident}");
                let (pat, fmt, args) = url.as_ref()?.expand(fields, &item_path);
                Some(quote! {
                    Self::#ident #pat => std::option::Option::Some(std::borrow::Cow::Owned(format!(#fmt #args))),
                })
            },
        )
    }

    pub(crate) fn gen_struct(&self, struct_name: &syn::Ident, fields: &Fields) -> TokenStream {
        let item_path = format!("struct.{struct_name}.html");
        let (pat, fmt, args) = self.expand(fields, &item_path);
        quote! {
            fn url(&self) -> std::option::Option<std::borrow::Cow<'_, str>> {
                #[allow(unused_variables, deprecated)]
                let Self #pat = self;
                std::option::Option::Some(std::borrow::Cow::Owned(format!(#fmt #args)))
            }
        }
    }
}
