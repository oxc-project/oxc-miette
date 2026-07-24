use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use rustc_hash::FxHashSet;
use syn::{
    Token, parenthesized,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    fmt::{self, Display},
    forward::WhichFn,
    utils::{display_pat_members, field_member, gen_all_variants_with, member_ident},
};

pub struct Labels(Vec<Label>);

#[derive(PartialEq, Eq)]
enum LabelType {
    Default,
    Primary,
    Collection,
}

struct Label {
    display: Option<Display>,
    ty: syn::Type,
    span: syn::Member,
    lbl_ty: LabelType,
}

impl Label {
    fn display_tokens(&self, members: &FxHashSet<syn::Member>) -> TokenStream {
        let Some(display) = &self.display else {
            return quote! { std::option::Option::None };
        };
        let (fmt, args) = display.expand_shorthand_cloned(members);
        quote! { std::option::Option::Some(format!(#fmt #args)) }
    }

    fn gen_label(
        &self,
        value: &TokenStream,
        display_members: &FxHashSet<syn::Member>,
    ) -> Option<TokenStream> {
        if self.lbl_ty == LabelType::Collection {
            return None;
        }
        let ty = &self.ty;
        let display = self.display_tokens(display_members);
        let constructor = if self.lbl_ty == LabelType::Primary {
            quote! { miette::LabeledSpan::new_primary_with_span }
        } else {
            quote! { miette::LabeledSpan::new_with_span }
        };
        let var = quote! { __miette_internal_var };
        Some(quote! {
            miette::macro_helpers::OptionalWrapper::<#ty>::new().to_option(#value)
                .map(|#var| #constructor(#display, #var.clone()))
        })
    }

    fn gen_collection(
        &self,
        value: &TokenStream,
        display_members: &FxHashSet<syn::Member>,
    ) -> Option<TokenStream> {
        if self.lbl_ty != LabelType::Collection {
            return None;
        }
        let display = self.display_tokens(display_members);
        Some(quote! {
            .chain({
                let display = #display;
                #value.iter().map(move |span| {
                    use miette::macro_helpers::{ToLabelSpanWrapper, ToLabeledSpan};
                    let mut labeled_span = ToLabelSpanWrapper::to_labeled_span(span.clone());
                    if display.is_some() && labeled_span.label().is_none() {
                        labeled_span.set_label(display.clone())
                    }
                    Some(labeled_span)
                })
            })
        })
    }
}

struct LabelAttr {
    label: Option<Display>,
    lbl_ty: LabelType,
}

impl Parse for LabelAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Skip a token.
        // This should receive one of:
        // - label = "..."
        // - label("...")
        let _ = input.step(|cursor| {
            if let Some((_, next)) = cursor.token_tree() {
                Ok(((), next))
            } else {
                Err(cursor.error("unexpected empty attribute"))
            }
        });
        let la = input.lookahead1();
        let (lbl_ty, label) = if la.peek(syn::token::Paren) {
            // #[label(primary?, "{}", x)]
            let content;
            parenthesized!(content in input);

            let attr = match content.parse::<Option<syn::Ident>>()? {
                Some(ident) if ident == "primary" => {
                    let _ = content.parse::<Token![,]>();
                    LabelType::Primary
                }
                Some(ident) if ident == "collection" => {
                    let _ = content.parse::<Token![,]>();
                    LabelType::Collection
                }
                Some(_) => {
                    return Err(syn::Error::new(
                        input.span(),
                        "Invalid argument to label() attribute. The argument must be a literal string or either the keyword `primary` or `collection`.",
                    ));
                }
                _ => LabelType::Default,
            };

            if content.peek(syn::LitStr) {
                let fmt = content.parse()?;
                let args = if content.is_empty() {
                    TokenStream::new()
                } else {
                    fmt::parse_token_expr(&content, false)?
                };
                let display = Display { fmt, args };
                (attr, Some(display))
            } else if !content.is_empty() {
                return Err(syn::Error::new(
                    input.span(),
                    "Invalid argument to label() attribute. The argument must be a literal string or either the keyword `primary` or `collection`.",
                ));
            } else {
                (attr, None)
            }
        } else if la.peek(Token![=]) {
            // #[label = "blabla"]
            input.parse::<Token![=]>()?;
            (LabelType::Default, Some(Display { fmt: input.parse()?, args: TokenStream::new() }))
        } else {
            (LabelType::Default, None)
        };
        Ok(LabelAttr { label, lbl_ty })
    }
}

impl Labels {
    pub fn from_fields(fields: &syn::Fields) -> syn::Result<Option<Self>> {
        let mut labels = Vec::new();
        for (i, field) in fields.iter().enumerate() {
            for attr in &field.attrs {
                if attr.path().is_ident("label") {
                    let span = field_member(i, field);
                    let LabelAttr { label, lbl_ty } =
                        syn::parse2::<LabelAttr>(attr.meta.to_token_stream())?;

                    if lbl_ty == LabelType::Primary
                        && labels.iter().any(|l: &Label| l.lbl_ty == LabelType::Primary)
                    {
                        return Err(syn::Error::new(
                            field.span(),
                            "Cannot have more than one primary label.",
                        ));
                    }

                    labels.push(Label { display: label, span, ty: field.ty.clone(), lbl_ty });
                }
            }
        }
        if labels.is_empty() { Ok(None) } else { Ok(Some(Labels(labels))) }
    }

    pub(crate) fn gen_struct(&self, fields: &syn::Fields) -> TokenStream {
        let (display_pat, display_members) = display_pat_members(fields);
        let labels = self.0.iter().filter_map(|label| {
            let span = &label.span;
            label.gen_label(&quote! { &self.#span }, &display_members)
        });
        let collections_chain = self.0.iter().filter_map(|label| {
            let span = &label.span;
            label.gen_collection(&quote! { self.#span }, &display_members)
        });

        quote! {
            #[allow(unused_variables)]
            fn labels(&self) -> miette::Labels {
                use miette::macro_helpers::ToOption;
                let Self #display_pat = self;

                let labels_iter = [
                    #(#labels),*
                ]
                .into_iter()
                #(#collections_chain)*;

                labels_iter.filter(Option::is_some).map(Option::unwrap).collect()
            }
        }
    }

    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::Labels,
            |ident, fields, DiagnosticConcreteArgs { labels, .. }| {
                let (display_pat, display_members) = display_pat_members(fields);
                labels.as_ref().and_then(|labels| {
                    let variant_labels = labels.0.iter().filter_map(|label| {
                        let field = member_ident(&label.span);
                        label.gen_label(&quote! { #field }, &display_members)
                    });
                    let collections_chain = labels.0.iter().filter_map(|label| {
                        let field = member_ident(&label.span);
                        label.gen_collection(&quote! { #field }, &display_members)
                    });
                    match &fields {
                        syn::Fields::Unit => None,
                        _ => Some(quote! {
                            Self::#ident #display_pat => {
                                use miette::macro_helpers::ToOption;
                                let labels_iter = [
                                    #(#variant_labels),*
                                ]
                                .into_iter()
                                #(#collections_chain)*;
                                labels_iter.filter(Option::is_some).map(Option::unwrap).collect()
                            }
                        }),
                    }
                })
            },
        )
    }
}
