//! Bullang compiler entry point.
//!
//! Global invocation (like tsc): run from anywhere inside a Bullang tree.
//! Bullang walks UP the directory tree to find the root inventory.bu,
//! then uses that directory as the project root regardless of rank.
//!
//! Usage:
//!   bullang build --name my_lib --ext rs
//!   bullang build --name my_lib --ext rs --out /path/to/output
//!   bullang check
//!   bullang file path/to/file.bu

mod ast;
mod build;
mod codegen;
mod parser;
mod typecheck;
mod validator;

use clap::{Parser as ClapParser, Subcommand};
use std::path::{Path, PathBuf};
use crate::ast::Backend;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(ClapParser)]
#[command(
    name    = "bullang",
    version = "0.1.0",
    about   = "Bullang (.bu) transpiler\n\n\
               Run from anywhere inside a Bullang project tree.\n\
               Bullang walks up to find the root automatically.\n\
               The source tree is never modified — all output is external."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate, type-check, and transpile the project into a standalone crate.
    Build {
        /// Name for the output project (folder name and crate name)
        #[arg(long)]
        name: String,

        /// Target language extension: 'rs' for Rust (more backends coming)
        #[arg(short = 'e', long)]
        ext: String,

        /// Output directory (default: sibling of the root, named after --name)
        #[arg(short = 'o', long)]
        out: Option<PathBuf>,
    },

    /// Validate and type-check without emitting any code.
    Check,

    /// Transpile a single .bu file to stdout or --output.
    /// No cross-file type checking — useful for quick inspection.
    File {
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { name, ext, out } => cmd_build(name, ext, out),
        Command::Check                    => cmd_check(),
        Command::File { input, output }   => cmd_file(input, output),
    }
}

// ── build ─────────────────────────────────────────────────────────────────────

fn cmd_build(name: String, ext: String, out: Option<PathBuf>) {
    let backend = Backend::from_ext(&ext).unwrap_or_else(|| {
        eprintln!("error: unknown extension '{}' — supported extensions: rs", ext);
        std::process::exit(1);
    });

    let root = find_root();

    let out_dir = match out {
        Some(p) => p,
        None    => root.parent().unwrap_or(&root).join(&name),
    };

    // Refuse to write inside the source tree
    if out_dir.starts_with(&root) {
        eprintln!(
            "error: output '{}' must be outside the source tree '{}'",
            out_dir.display(), root.display()
        );
        std::process::exit(1);
    }

    let root_rank = validator::read_folder_rank(&root)
        .expect("root has no rank — this should not happen after find_root()");

    println!("bullang build");
    println!("  root    : {} ({})", root.display(), root_rank.name());
    println!("  output  : {}", out_dir.display());
    println!("  name    : {}", name);
    println!("  backend : {}", backend.name());
    println!();

    // Phase 1: structural validation
    let errors = validator::validate_tree(&root);
    if !errors.is_empty() {
        for e in &errors { eprintln!("error: {}", e); }
        eprintln!("\n{} structural error(s) — build aborted", errors.len());
        std::process::exit(1);
    }
    println!("structural validation ... ok");

    // Phase 2: type checking
    let type_errors = typecheck::typecheck_tree(&root);
    if !type_errors.is_empty() {
        for e in &type_errors { eprintln!("type error: {}", e); }
        eprintln!("\n{} type error(s) — build aborted", type_errors.len());
        std::process::exit(1);
    }
    println!("type checking         ... ok");

    // Phase 3: code generation
    let result = build::build(&root, &out_dir, &name, &backend);
    for e in &result.errors { eprintln!("error: {}", e); }
    if !result.errors.is_empty() {
        eprintln!("\nbuild failed -- {} error(s)", result.errors.len());
        std::process::exit(1);
    }

    println!("code generation       ... ok");
    println!();
    println!("wrote {} file(s) to {}", result.files_written, out_dir.display());
    println!();
    match backend {
        Backend::Rust => {
            println!("to compile:");
            println!("  cd {} && cargo build", out_dir.display());
        }
    }
}

// ── check ─────────────────────────────────────────────────────────────────────

fn cmd_check() {
    let root = find_root();
    let root_rank = validator::read_folder_rank(&root)
        .expect("root has no rank");

    println!("bullang check -- root: {} ({})", root.display(), root_rank.name());

    let errors = validator::validate_tree(&root);
    if !errors.is_empty() {
        for e in &errors { eprintln!("error: {}", e); }
        eprintln!("\n{} structural error(s) — fix before type checking", errors.len());
        std::process::exit(1);
    }

    let type_errors = typecheck::typecheck_tree(&root);
    if type_errors.is_empty() {
        println!("ok -- no errors found");
    } else {
        for e in &type_errors { eprintln!("type error: {}", e); }
        eprintln!("\n{} type error(s) found", type_errors.len());
        std::process::exit(1);
    }
}

// ── file ──────────────────────────────────────────────────────────────────────

fn cmd_file(input: PathBuf, output: Option<PathBuf>) {
    let source = read_file(&input);

    let is_inventory = input.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "inventory.bu")
        .unwrap_or(false);

    let bu = parser::parse_file(&source, is_inventory).unwrap_or_else(|e| {
        eprintln!("parse error in {}:\n  {}", input.display(), e);
        std::process::exit(1);
    });

    match bu {
        ast::BuFile::Skirmish(ref sk) => {
            use std::collections::HashSet;
            let path = input.display().to_string();

            let errors = validator::validate_bu_file_direct(
                sk, &path, &HashSet::new(), &sk.rank,
            );
            if !errors.is_empty() {
                for e in &errors { eprintln!("error: {}", e); }
                std::process::exit(1);
            }

            let type_errors = typecheck::typecheck_file(sk, &path);
            if !type_errors.is_empty() {
                for e in &type_errors { eprintln!("type error: {}", e); }
                std::process::exit(1);
            }

            write_or_print(codegen::emit_skirmish(sk), output);
        }
        ast::BuFile::Inventory(_) => {
            write_or_print(codegen::emit_mod_rs(&[], &[]), output);
        }
    }
}

// ── Root detection ────────────────────────────────────────────────────────────

/// Walk UP from the current directory to find the Bullang project root.
///
/// The root is the HIGHEST ancestor directory that still contains an
/// inventory.bu file. This mirrors how tsc finds tsconfig.json —
/// you can run `bullang` from anywhere inside the tree.
///
/// Algorithm:
///   1. Start at cwd.
///   2. Keep moving to the parent as long as the parent also has inventory.bu.
///   3. Stop when the parent has no inventory.bu — current dir is the root.
///
/// If cwd itself has no inventory.bu, print a helpful error and exit.
fn find_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("error: could not determine current directory: {}", e);
        std::process::exit(1);
    });

    if !cwd.join("inventory.bu").exists() {
        eprintln!(
            "error: no inventory.bu found in '{}'\n\
             run bullang from inside a Bullang project directory",
            cwd.display()
        );
        std::process::exit(1);
    }

    // Walk up as long as the parent also has inventory.bu
    let mut root = cwd.clone();
    loop {
        let parent = match root.parent() {
            Some(p) => p.to_path_buf(),
            None    => break,
        };
        if parent.join("inventory.bu").exists() {
            root = parent;
        } else {
            break;
        }
    }

    // Validate that the found root has a valid rank
    match validator::read_folder_rank(&root) {
        Some(_) => {}
        None => {
            eprintln!(
                "error: found inventory.bu at '{}' but could not read its #rank",
                root.display()
            );
            std::process::exit(1);
        }
    }

    root
}

/// Check if a directory looks like a Bullang folder of a specific rank.
/// Kept for potential future use in diagnostics.
#[allow(dead_code)]
fn is_bu_folder(dir: &Path) -> bool {
    dir.join("inventory.bu").exists()
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn read_file(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error reading {}: {}", path.display(), e);
        std::process::exit(1);
    })
}

fn write_or_print(content: String, output: Option<PathBuf>) {
    match output {
        Some(ref p) => std::fs::write(p, &content).unwrap_or_else(|e| {
            eprintln!("error writing {}: {}", p.display(), e);
            std::process::exit(1);
        }),
        None => print!("{}", content),
    }
}
