// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! `bamboo-native-ui-layout-codegen <input.buixml> [-o out.rs] [--module name] [--context Type]`
//!
//! Turns a layout skin into the compiled Rust module (D128). The output is
//! ordinary source: check it in and `include!` it, or generate it from a build
//! script. Exit status is 1 on a document error (the diagnostics print with
//! the `.buixml` position).

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut module: Option<String> = None;
    let mut context: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => output = args.next(),
            "--module" => module = args.next(),
            "--context" => context = args.next(),
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown option `{other}`");
                print_help();
                return ExitCode::FAILURE;
            }
            other => {
                if input.replace(other.to_string()).is_some() {
                    eprintln!("more than one input file");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    let Some(input) = input else {
        print_help();
        return ExitCode::FAILURE;
    };
    let source = match std::fs::read_to_string(&input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("cannot read {input}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let name = std::path::Path::new(&input)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| input.clone());
    let module = module.unwrap_or_else(|| bamboo_native_ui_layout_codegen::module_name(&name));
    match bamboo_native_ui_layout_codegen::generate_with(
        &source,
        &name,
        bamboo_native_ui_layout_codegen::Options {
            module: &module,
            context: context.as_deref(),
        },
    ) {
        Ok(generated) => {
            match output {
                Some(path) => {
                    if let Err(error) = std::fs::write(&path, generated) {
                        eprintln!("cannot write {path}: {error}");
                        return ExitCode::FAILURE;
                    }
                    println!("{path}: {name} -> mod {module}");
                }
                None => print!("{generated}"),
            }
            ExitCode::SUCCESS
        }
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{name}: {diagnostic}");
            }
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    eprintln!(
        "usage: bamboo-native-ui-layout-codegen <input.buixml> [-o out.rs] [--module name] [--context Type]"
    );
}
