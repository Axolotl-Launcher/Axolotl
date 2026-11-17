use std::fs;
use std::path::{Path, PathBuf};

use syn::Attribute;

/// Every Tauri command must carry a tracing span so the IPC surface is
/// observable. `skip_all` keeps command arguments, which routinely contain
/// paths, tokens and account data, out of the span.
#[test]
fn every_tauri_command_is_instrumented() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&source_root, &mut files);
    files.sort();

    let mut command_count = 0;
    let mut failures = Vec::new();
    for file in &files {
        let source = fs::read_to_string(file).unwrap_or_else(|error| {
            panic!("cannot read {}: {error}", file.display())
        });
        let parsed = syn::parse_file(&source).unwrap_or_else(|error| {
            panic!("cannot parse {}: {error}", file.display())
        });
        inspect_items(&parsed.items, file, &mut command_count, &mut failures);
    }

    assert!(
        command_count > 0,
        "no #[tauri::command] functions found under {}",
        source_root.display()
    );
    assert!(
        failures.is_empty(),
        "{} of {command_count} #[tauri::command] functions do not carry \
         exactly one #[tracing::instrument(skip_all, ...)] attribute:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory).unwrap_or_else(|error| {
        panic!("cannot read {}: {error}", directory.display())
    });
    for entry in entries {
        let entry = entry.expect("cannot read directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn inspect_items(
    items: &[syn::Item],
    file: &Path,
    command_count: &mut usize,
    failures: &mut Vec<String>,
) {
    for item in items {
        match item {
            syn::Item::Fn(function) => {
                if !function.attrs.iter().any(is_command_attribute) {
                    continue;
                }
                *command_count += 1;
                let name = function.sig.ident.clone();
                let instruments: Vec<&Attribute> = function
                    .attrs
                    .iter()
                    .filter(|attribute| is_instrument_attribute(attribute))
                    .collect();
                if instruments.len() != 1 {
                    failures.push(format!(
                        "{}: {name} has {} tracing instrument attributes",
                        file.display(),
                        instruments.len()
                    ));
                } else if !has_skip_all(instruments[0]) {
                    failures.push(format!(
                        "{}: {name} tracing instrument attribute lacks skip_all",
                        file.display()
                    ));
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    inspect_items(nested, file, command_count, failures);
                }
            }
            _ => {}
        }
    }
}

fn is_command_attribute(attribute: &Attribute) -> bool {
    attribute_path(attribute) == ["tauri", "command"]
}

fn is_instrument_attribute(attribute: &Attribute) -> bool {
    attribute_path(attribute) == ["tracing", "instrument"]
}

fn attribute_path(attribute: &Attribute) -> Vec<String> {
    attribute
        .path()
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn has_skip_all(attribute: &Attribute) -> bool {
    let mut skip_all = false;
    let _ = attribute.parse_nested_meta(|meta| {
        if meta.path.is_ident("skip_all") {
            skip_all = true;
        } else if meta.input.peek(syn::Token![=]) {
            let _: syn::Expr = meta.value()?.parse()?;
        }
        Ok(())
    });
    skip_all
}
