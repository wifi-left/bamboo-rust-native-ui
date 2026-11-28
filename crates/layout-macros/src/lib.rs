// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! `layout_xml!` — compile a `.buixml` skin into Rust at build time (D128).
//!
//! ```ignore
//! bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml");
//! // defines `pub mod main { pub static LAYOUT; pub fn install(..); }`
//!
//! bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml", module = app_layout);
//! // the install takes `context: &AppLayout` and inline Rust may read it
//! bamboo_native_ui_layout_macros::layout_xml!("layouts/main.buixml", context = AppLayout);
//! ```
//!
//! The macro reads the file relative to `CARGO_MANIFEST_DIR`, parses it with
//! `bamboo-native-ui-layout-core`, generates the same module as the CLI
//! (`bamboo-native-ui-layout-codegen`) and expands it in place. A document
//! error becomes a `compile_error!` naming the `.buixml` line and column.
//!
//! Caveats:
//! - Stable proc macros cannot register a file dependency, so editing the
//!   `.buixml` file does not by itself trigger a rebuild; use the CLI or a
//!   build script when the document changes independently (the golden test in
//!   the codegen crate is the CI check).
//! - Inline Rust sees the including module (`use super::*`), in geometry
//!   `cw`/`ch`/`cx`/`cy`, and — with `context = Type` — the `context`
//!   parameter of the generated `install`.

use std::path::PathBuf;

use proc_macro::{TokenStream, TokenTree};

/// Expand a `.buixml` file into the compiled layout module.
#[proc_macro]
pub fn layout_xml(input: TokenStream) -> TokenStream {
    match expand(input.into_iter().collect()) {
        Ok(source) => match source.parse() {
            Ok(tokens) => tokens,
            Err(error) => compile_error(&format!("layout_xml!: generated Rust does not parse: {error}")),
        },
        Err(message) => compile_error(&message),
    }
}

fn expand(tokens: Vec<TokenTree>) -> Result<String, String> {
    let mut iter = tokens.into_iter();
    let Some(TokenTree::Literal(path_literal)) = iter.next() else {
        return Err(
            "layout_xml!: expected a string path, e.g. layout_xml!(\"layouts/main.buixml\")"
                .to_string(),
        );
    };
    let path = literal_text(&path_literal.to_string())?;
    let tokens: Vec<TokenTree> = iter.collect();

    let mut module = None;
    let mut context = None;
    let mut position = 0;
    while position < tokens.len() {
        match &tokens[position] {
            TokenTree::Punct(punct) if punct.as_char() == ',' => position += 1,
            TokenTree::Ident(ident) => {
                let key = ident.to_string();
                if position + 2 >= tokens.len() {
                    return Err(format!(
                        "layout_xml!: `{key}` needs a value, e.g. `{key} = …`"
                    ));
                }
                match (&tokens[position + 1], key.as_str()) {
                    (TokenTree::Punct(punct), _) if punct.as_char() != '=' => {
                        return Err(format!("layout_xml!: expected `{key} = …`"));
                    }
                    (_, "module") => match &tokens[position + 2] {
                        TokenTree::Ident(value) => {
                            module = Some(value.to_string());
                            position += 3;
                        }
                        _ => return Err("layout_xml!: expected `module = name`".to_string()),
                    },
                    (_, "context") => {
                        let (text, consumed) = type_path(&tokens[position + 2..])?;
                        context = Some(text);
                        position += 2 + consumed;
                    }
                    (_, other) => {
                        return Err(format!(
                            "layout_xml!: unsupported argument `{other}`; `module = name` and `context = Type` are accepted"
                        ));
                    }
                }
            }
            other => {
                return Err(format!("layout_xml!: unsupported token `{other}`"));
            }
        }
    }

    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "layout_xml!: CARGO_MANIFEST_DIR is not set".to_string())?;
    let file = PathBuf::from(&manifest).join(&path);
    let source = std::fs::read_to_string(&file).map_err(|error| {
        format!(
            "layout_xml!: cannot read {}: {error}",
            file.display()
        )
    })?;
    let file_name = PathBuf::from(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    let module = module.unwrap_or_else(|| bamboo_native_ui_layout_codegen::module_name(&file_name));

    bamboo_native_ui_layout_codegen::generate_with(
        &source,
        &file_name,
        bamboo_native_ui_layout_codegen::Options {
            module: &module,
            context: context.as_deref(),
        },
    )
    .map_err(|diagnostics| {
        let mut message = format!("layout_xml!: {} has {} problem(s):", path, diagnostics.len());
        for diagnostic in diagnostics {
            message.push_str(&format!("\n  {diagnostic}"));
        }
        message
    })
}

/// Collect one Rust type path (`AppLayout`, `crate::ui::Ctx<'_>`) from a token
/// slice, stopping at a top-level comma. Returns the source text and how many
/// tokens were consumed. Joint punctuation (`::`, `<'a>`) is re-glued so the
/// text still parses as Rust.
fn type_path(tokens: &[TokenTree]) -> Result<(String, usize), String> {
    let mut depth = 0usize;
    let mut collected = 0usize;
    let mut text = String::new();
    let mut glue = false;
    for token in tokens {
        let mut drop = false;
        match token {
            TokenTree::Punct(punct) => {
                let ch = punct.as_char();
                match ch {
                    '<' | '(' | '[' => depth += 1,
                    '>' | ')' | ']' => depth = depth.saturating_sub(1),
                    ',' if depth == 0 => drop = true,
                    _ => {}
                }
                if !drop {
                    if !glue && !text.is_empty() {
                        text.push(' ');
                    }
                    text.push(ch);
                    glue = punct.spacing() == proc_macro::Spacing::Joint;
                }
            }
            TokenTree::Group(group) => {
                if !glue && !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&group.to_string());
                glue = false;
            }
            other => {
                if !glue && !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&other.to_string());
                glue = false;
            }
        }
        if drop {
            break;
        }
        collected += 1;
    }
    if collected == 0 {
        return Err("layout_xml!: `context` needs a type, e.g. `context = AppLayout`".to_string());
    }
    Ok((text, collected))
}

/// The value of a string literal token (`"text"`), rejecting escapes for paths.
fn literal_text(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let Some(inner) = trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return Err("layout_xml!: the first argument must be a plain string literal".to_string());
    };
    if inner.contains('\\') {
        return Err(
            "layout_xml!: use forward slashes in the path; escapes are not supported".to_string(),
        );
    }
    if inner.is_empty() {
        return Err("layout_xml!: the path is empty".to_string());
    }
    Ok(inner.to_string())
}

fn compile_error(message: &str) -> TokenStream {
    let escaped = message
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("compile_error!(\"{escaped}\");")
        .parse()
        .expect("compile_error! expands")
}
