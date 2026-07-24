use diagnostic::Diagnostic;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::quote;
use std::path::{Path, PathBuf};
use syn::{DeriveInput, Ident, parse_macro_input};
use toml_edit::{DocumentMut, TableLike};

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

fn dependency_names_package(
    key: &str,
    dependency: &toml_edit::Item,
    package: &str,
    workspace_dependencies: Option<&dyn TableLike>,
) -> bool {
    let dependency = dependency.as_table_like();
    if let Some(declared_package) =
        dependency.and_then(|dependency| dependency.get("package")?.as_str())
    {
        return declared_package == package;
    }

    let inherited =
        dependency.and_then(|dependency| dependency.get("workspace")?.as_bool()).unwrap_or(false);
    if inherited {
        return workspace_dependencies.and_then(|dependencies| dependencies.get(key)).is_some_and(
            |dependency| {
                dependency
                    .as_table_like()
                    .and_then(|dependency| dependency.get("package")?.as_str())
                    .unwrap_or(key)
                    == package
            },
        );
    }

    key == package
}

fn dependency_key_in_section(
    table: &dyn TableLike,
    section: &str,
    package: &str,
    workspace_dependencies: Option<&dyn TableLike>,
) -> Option<String> {
    table.get(section)?.as_table_like()?.iter().find_map(|(key, dependency)| {
        dependency_names_package(key, dependency, package, workspace_dependencies)
            .then(|| key.to_owned())
    })
}

fn target_dependency_keys_in_section(
    manifest: &DocumentMut,
    section: &str,
    package: &str,
    workspace_dependencies: Option<&dyn TableLike>,
) -> Vec<(String, String)> {
    manifest
        .get("target")
        .and_then(toml_edit::Item::as_table_like)
        .into_iter()
        .flat_map(TableLike::iter)
        .filter_map(|(target, table)| {
            let key = dependency_key_in_section(
                table.as_table_like()?,
                section,
                package,
                workspace_dependencies,
            )?;
            Some((target.to_owned(), key))
        })
        .collect()
}

fn workspace_dependencies(manifest: &DocumentMut) -> Option<&dyn TableLike> {
    manifest.get("workspace")?.as_table_like()?.get("dependencies")?.as_table_like()
}

fn read_manifest(path: &Path) -> Option<DocumentMut> {
    std::fs::read_to_string(path).ok()?.parse().ok()
}

fn inherited_workspace_manifest(
    manifest_dir: &Path,
    manifest: &DocumentMut,
) -> Option<DocumentMut> {
    if let Some(workspace) = manifest
        .get("package")
        .and_then(toml_edit::Item::as_table_like)
        .and_then(|package| package.get("workspace"))
        .and_then(toml_edit::Item::as_str)
    {
        return read_manifest(&manifest_dir.join(workspace).join("Cargo.toml"));
    }

    manifest_dir.parent()?.ancestors().find_map(|ancestor| {
        let manifest = read_manifest(&ancestor.join("Cargo.toml"))?;
        manifest.get("workspace").is_some().then_some(manifest)
    })
}

fn target_cfg(target: &str) -> Option<proc_macro2::TokenStream> {
    if let Some(cfg) = target.strip_prefix("cfg(").and_then(|cfg| cfg.strip_suffix(')')) {
        return cfg.parse().ok();
    }

    let target = cfg_expr::targets::get_builtin_target_by_triple(target)?;
    let arch = target.arch.as_str();
    let os = target.os.as_ref().map_or("none", cfg_expr::targets::Os::as_str);
    let abi = target.abi.as_ref().map_or("", cfg_expr::targets::Abi::as_str);
    let env = target.env.as_ref().map_or("", cfg_expr::targets::Env::as_str);
    let vendor = target.vendor.as_ref().map_or("unknown", cfg_expr::targets::Vendor::as_str);
    let pointer_width = target.pointer_width.to_string();
    let endian = match target.endian {
        cfg_expr::targets::Endian::big => "big",
        cfg_expr::targets::Endian::little => "little",
    };
    Some(quote! {
        all(
            target_arch = #arch,
            target_os = #os,
            target_abi = #abi,
            target_env = #env,
            target_vendor = #vendor,
            target_pointer_width = #pointer_width,
            target_endian = #endian
        )
    })
}

fn dependency_is_available(key: &str) -> bool {
    let key = if key == "oxc-miette" { "miette".to_owned() } else { key.replace('-', "_") };
    let mut arguments = std::env::args();
    while let Some(argument) = arguments.next() {
        let external = if argument == "--extern" {
            arguments.next()
        } else {
            argument.strip_prefix("--extern=").map(str::to_owned)
        };
        if external.as_deref().and_then(|external| external.split('=').next()) == Some(&key) {
            return true;
        }
    }
    false
}

fn manifest_dependency_import(
    manifest: &DocumentMut,
    package: &str,
    sections: &[&str],
    workspace_dependencies: Option<&dyn TableLike>,
) -> Option<proc_macro2::TokenStream> {
    sections.iter().find_map(|section| {
        if let Some(key) =
            dependency_key_in_section(manifest.as_table(), section, package, workspace_dependencies)
        {
            let path = dependency_path(&key);
            return Some(quote!(use #path as miette;));
        }

        let dependencies =
            target_dependency_keys_in_section(manifest, section, package, workspace_dependencies);
        if let Some((_, key)) = dependencies.iter().find(|(_, key)| dependency_is_available(key)) {
            let path = dependency_path(key);
            return Some(quote!(use #path as miette;));
        }

        let imports = dependencies
            .into_iter()
            .filter_map(|(target, key)| {
                let cfg = target_cfg(&target)?;
                let path = dependency_path(&key);
                Some(quote! {
                    #[cfg(#cfg)]
                    use #path as miette;
                })
            })
            .collect::<Vec<_>>();
        (!imports.is_empty()).then(|| quote!(#(#imports)*))
    })
}

fn current_manifest_dependency_import(package: &str) -> Option<proc_macro2::TokenStream> {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")?);
    let manifest = read_manifest(&manifest_dir.join("Cargo.toml"))?;
    let workspace_manifest = inherited_workspace_manifest(&manifest_dir, &manifest);
    let workspace_dependencies = workspace_dependencies(&manifest)
        .or_else(|| workspace_manifest.as_ref().and_then(workspace_dependencies));
    let sections = if std::env::var("CARGO_CRATE_NAME").as_deref() == Ok("build_script_build") {
        &["build-dependencies"][..]
    } else {
        &["dependencies", "dev-dependencies"][..]
    };
    manifest_dependency_import(&manifest, package, sections, workspace_dependencies)
}

fn dependency_path(key: &str) -> proc_macro2::TokenStream {
    if key == "oxc-miette" {
        return quote!(::miette);
    }
    let ident = Ident::new(&key.replace('-', "_"), Span::call_site());
    quote!(::#ident)
}

#[proc_macro_derive(
    Diagnostic,
    attributes(diagnostic, source_code, label, related, help, diagnostic_source)
)]
pub fn derive_diagnostic(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let miette_import = if std::env::var("CARGO_PKG_NAME").as_deref() == Ok("oxc-miette") {
        quote!(
            use ::miette;
        )
    } else if let Some(import) = current_manifest_dependency_import("oxc-miette") {
        import
    } else {
        match crate_name("oxc-miette") {
            Ok(FoundCrate::Itself) => quote!(
                use crate as miette;
            ),
            Ok(FoundCrate::Name(name)) => {
                let ident = Ident::new(&name, Span::call_site());
                quote!(use ::#ident as miette;)
            }
            Err(_) => quote!(
                use ::miette;
            ),
        }
    };
    let cmd = match Diagnostic::from_derive_input(input) {
        Ok(cmd) => cmd.r#gen(),
        Err(err) => return err.to_compile_error().into(),
    };
    quote! {
        const _: () = {
            #miette_import
            #cmd
        };
    }
    .into()
}
