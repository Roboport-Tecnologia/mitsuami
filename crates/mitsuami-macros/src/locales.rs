//! `locales!`: a folder of Fluent files, checked and built into the app.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fluent_syntax::ast::Entry;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token};

struct Input {
    dir: LitStr,
    fallback: Option<LitStr>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Input> {
        let dir = input.parse()?;
        let mut fallback = None;
        while input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
            let name: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            match name.to_string().as_str() {
                "fallback" => fallback = Some(input.parse()?),
                _ => return Err(syn::Error::new(name.span(), "expected `fallback = \"…\"`")),
            }
        }
        Ok(Input { dir, fallback })
    }
}

/// `file` is the calling file's, where the compiler knows it: the path is
/// from its folder, as `include_str!`'s. Without it (rust-analyzer), from
/// the crate's `Cargo.toml`.
pub(crate) fn expand(input: TokenStream, file: Option<PathBuf>) -> syn::Result<TokenStream> {
    let Input { dir, fallback } = syn::parse2(input)?;
    let span = dir.span();
    let error = |message: String| syn::Error::new(span, message);
    let from = match file.as_deref().and_then(Path::parent) {
        // Relative to where the compiler runs.
        Some(folder) => std::env::current_dir().map(|cwd| cwd.join(folder)).unwrap_or_else(|_| folder.to_owned()),
        None => std::env::var("CARGO_MANIFEST_DIR").map_err(|_| error("CARGO_MANIFEST_DIR isn't set".into()))?.into(),
    };
    let base = from.join(dir.value());
    // Only tools that don't give the calling file (rust-analyzer) get
    // here, and the path is likely from that file: no false error.
    if file.is_none() && !base.is_dir() {
        return Ok(quote! { ::mitsuami::core::l10n::Locales::new() });
    }
    let languages = read_dir(&base).map_err(|e| error(format!("can't read {}: {e}", base.display())))?;
    let mut files: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for language in languages.into_iter().filter(|p| p.is_dir()) {
        let tag = language.file_name().unwrap().to_string_lossy().into_owned();
        if tag.parse::<unic_langid::LanguageIdentifier>().is_err() {
            return Err(error(format!("{}: `{tag}` isn't a language tag (en-US, pt-BR)", language.display())));
        }
        let ftl: Vec<PathBuf> = read_dir(&language)
            .map_err(|e| error(format!("can't read {}: {e}", language.display())))?
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "ftl"))
            .collect();
        files.insert(tag, ftl);
    }
    if files.is_empty() {
        return Err(error(format!("{} has no language folders (en-US/app.ftl, …)", base.display())));
    }
    let fallback = match fallback {
        Some(fallback) if files.contains_key(&fallback.value()) => fallback.value(),
        Some(fallback) => {
            return Err(syn::Error::new(
                fallback.span(),
                format!("{} has no `{}` folder", base.display(), fallback.value()),
            ));
        }
        None if files.contains_key("en-US") => "en-US".into(),
        None if files.len() == 1 => files.keys().next().unwrap().clone(),
        None => return Err(error("which language is the fallback? `locales!(\"…\", fallback = \"en-US\")`".into())),
    };
    let mut adds = Vec::new();
    for (tag, paths) in &files {
        let mut ids: BTreeMap<String, PathBuf> = BTreeMap::new();
        for path in paths {
            let source =
                std::fs::read_to_string(path).map_err(|e| error(format!("can't read {}: {e}", path.display())))?;
            let resource = match fluent_syntax::parser::parse(source.as_str()) {
                Ok(resource) => resource,
                Err((_, errors)) => {
                    let first = &errors[0];
                    let line = source[..first.pos.start.min(source.len())].lines().count().max(1);
                    return Err(error(format!(
                        "{}:{line}: {} ({} error{} in all)",
                        path.display(),
                        first.kind,
                        errors.len(),
                        if errors.len() == 1 { "" } else { "s" }
                    )));
                }
            };
            // The same message twice in a language: one would be lost.
            for entry in &resource.body {
                let id = match entry {
                    Entry::Message(m) => m.id.name.to_string(),
                    Entry::Term(t) => format!("-{}", t.id.name),
                    _ => continue,
                };
                if let Some(other) = ids.insert(id.clone(), path.clone()) {
                    return Err(error(format!("`{id}` is in both {} and {} ({tag})", other.display(), path.display())));
                }
            }
            let name = format!("{tag}/{}", path.file_name().unwrap().to_string_lossy());
            let absolute = path.to_string_lossy().into_owned();
            let absolute = LitStr::new(&absolute, Span::call_site());
            adds.push(quote! { .add_file(#tag, #name, ::core::include_str!(#absolute)) });
        }
    }
    Ok(quote! {
        ::mitsuami::core::l10n::Locales::new().fallback(#fallback) #(#adds)*
    })
}

fn read_dir(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>()?;
    entries.sort();
    Ok(entries)
}
