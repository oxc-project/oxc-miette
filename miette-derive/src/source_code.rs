use crate::{
    diagnostic::{DiagnosticConcreteArgs, DiagnosticDef},
    forward::WhichFn,
    utils::{display_pat_members, field_member, find_attr, gen_all_variants_with, member_ident},
};
use proc_macro2::TokenStream;
use quote::quote;

pub struct SourceCode {
    source_code: syn::Member,
    is_option: bool,
}

impl SourceCode {
    pub fn from_fields(fields: &syn::Fields) -> Option<Self> {
        let (index, field) = find_attr(fields, "source_code")?;
        let is_option = matches!(
            &field.ty,
            syn::Type::Path(syn::TypePath { path, .. })
                if path.segments.last().is_some_and(|segment| segment.ident == "Option")
        );
        Some(Self { source_code: field_member(index, field), is_option })
    }

    pub(crate) fn gen_struct(&self, fields: &syn::Fields) -> TokenStream {
        let (display_pat, _display_members) = display_pat_members(fields);
        let src = &self.source_code;
        let ret = if self.is_option {
            quote! {
                self.#src.as_ref().map(|s| s as _)
            }
        } else {
            quote! {
                Some(&self.#src)
            }
        };

        quote! {
            #[allow(unused_variables)]
            fn source_code(&self) -> std::option::Option<&dyn miette::SourceCode> {
                let Self #display_pat = self;
                #ret
            }
        }
    }

    pub(crate) fn gen_enum(variants: &[DiagnosticDef]) -> Option<TokenStream> {
        gen_all_variants_with(
            variants,
            WhichFn::SourceCode,
            |ident, fields, DiagnosticConcreteArgs { source_code, .. }| {
                let (display_pat, _display_members) = display_pat_members(fields);
                source_code.as_ref().and_then(|source_code| {
                    let field = member_ident(&source_code.source_code);
                    let ret = if source_code.is_option {
                        quote! {
                            #field.as_ref().map(|s| s as _)
                        }
                    } else {
                        quote! {
                            std::option::Option::Some(#field)
                        }
                    };
                    match &fields {
                        syn::Fields::Unit => None,
                        _ => Some(quote! {
                            Self::#ident #display_pat => #ret,
                        }),
                    }
                })
            },
        )
    }
}
