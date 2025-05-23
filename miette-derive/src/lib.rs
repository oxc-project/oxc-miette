use diagnostic::Diagnostic;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

mod code;
mod diagnostic;
mod diagnostic_arg;
mod diagnostic_source;
mod fmt;
mod forward;
mod help;
mod label;
mod related;
mod severity;
mod source_code;
mod url;
mod utils;

#[proc_macro_derive(
    Diagnostic,
    attributes(diagnostic, source_code, label, related, help, diagnostic_source)
)]
pub fn derive_diagnostic(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let cmd = match Diagnostic::from_derive_input(input) {
        Ok(cmd) => cmd.r#gen(),
        Err(err) => return err.to_compile_error().into(),
    };
    // panic!("{:#}", cmd.to_token_stream());
    quote!(#cmd).into()
}
