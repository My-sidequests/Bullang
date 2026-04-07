//! Bullang compiler entry point.
//!
//! Usage (run from inside the war root directory):
//!
//!   bullang build -name my_program -ext rs
//!   bullang build -name my_program -ext rs -out /path/to/output
//!   bullang check
//!   bullang file path/to/file.bu
//!
//! Future: `bullang build` will be runnable from anywhere (like tsc),
//! walking up the directory tree to find the war root automatically.

mod ast;
mod build;
mod codegen;
mod parser;
mod typecheck;
mod validator;

use clap::{Parser as ClapParser, Subcommand};
use std::path::PathBuf;
use crate::ast::Backend;

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(ClapParser)]
#[command(
    name    = "bullang",
    version = "0.1.0",
    about   = "Bullang (.bu) transpiler\n\n\
               Run from inside the war root directory.\n\
               The source tree is never modified — all output is external."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate and type-check the war tree, then transpile it into a project.
    /// Run from inside the war root directory.
    Build {
        /// Name for the output project (becomes the folder name and crate name)
        #[arg(long)]
        name: String,

        /// Target language extension: 'rs' for Rust (more backends coming)
        #[arg(short = 'e', long)]
        ext: String,

        /// Output directory (default: ../\<name\>, a sibling of the war root)
        #[arg(short = 'o', long)]
        out: Option<PathBuf>,
    },

    /// Validate and type-check the war tree without emitting any code.
    /// Run from inside the war root directory.
    Check,

    /// Transpile a single .bu file to stdout (or --output).
    /// Useful for quick inspection — no cross-file type checking.
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
    // Resolve backend from extension
    let backend = Backend::from_ext(&ext).unwrap_or_else(|| {
        eprintln!(
            "error: unknown extension '{}' — supported: rs",
            ext
        );
        std::process::exit(1);
    });

    // War root = current working directory
    let war_root = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("error: could not determine current directory: {}", e);
        std::process::exit(1);
    });

    guard_war_root(&war_root);

    // Output directory: --out if given, otherwise ../name (sibling of war root)
    let out_dir = match out {
        Some(p) => p,
        None => war_root
            .parent()
            .unwrap_or(&war_root)
            .join(&name),
    };

    // Refuse to write inside the source tree
    if out_dir.starts_with(&war_root) {
        eprintln!(
            "error: output '{}' must be outside the war source tree '{}'",
            out_dir.display(), war_root.display()
        );
        std::process::exit(1);
    }

    println!("bullang build");
    println!("  source  : {}", war_root.display());
    println!("  output  : {}", out_dir.display());
    println!("  name    : {}", name);
    println!("  backend : {}", backend.name());
    println!();

    // Phase 1: structural validation
    let errors = validator::validate_tree(&war_root);
    if !errors.is_empty() {
        for e in &errors { eprintln!("error: {}", e); }
        eprintln!("\n{} structural error(s) — build aborted", errors.len());
        std::process::exit(1);
    }
    println!("structural validation ... ok");

    // Phase 2: type checking
    let type_errors = typecheck::typecheck_tree(&war_root);
    if !type_errors.is_empty() {
        for e in &type_errors { eprintln!("type error: {}", e); }
        eprintln!("\n{} type error(s) — build aborted", type_errors.len());
        std::process::exit(1);
    }
    println!("type checking         ... ok");

    // Phase 3: code generation
    let result = build::build(&war_root, &out_dir, &name, &backend);
    for e in &result.errors { eprintln!("error: {}", e); }
    if !result.errors.is_empty() {
        eprintln!("\nbuild failed -- {} error(s)", result.errors.len());
        std::process::exit(1);
    }

    println!("code generation       ... ok");
    println!();
    println!("wrote {} file(s) to {}", result.files_written, out_dir.display());
    println!();

    // Backend-specific next-step hint
    match backend {
        Backend::Rust => {
            println!("to compile:");
            println!("  cd {} && cargo build", out_dir.display());
        }
    }
}

// ── check ─────────────────────────────────────────────────────────────────────

fn cmd_check() {
    let war_root = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("error: could not determine current directory: {}", e);
        std::process::exit(1);
    });

    guard_war_root(&war_root);

    // Phase 1: structural validation
    let errors = validator::validate_tree(&war_root);
    if !errors.is_empty() {
        for e in &errors { eprintln!("error: {}", e); }
        eprintln!("\n{} structural error(s) — fix before type checking",
            errors.len());
        std::process::exit(1);
    }

    // Phase 2: type checking
    let type_errors = typecheck::typecheck_tree(&war_root);
    if type_errors.is_empty() {
        println!("ok -- {} validated and type-checked with no errors",
            war_root.display());
    } else {
        for e in &type_errors { eprintln!("type error: {}", e); }
        eprintln!("\n{} type error(s) found", type_errors.len());
        std::process::exit(1);
    }
}

// ── file ──────────────────────────────────────────────────────────────────────

fn cmd_file(input: PathBuf, output: Option<PathBuf>) {
    let source = read_file(&input);

    let is_inventory = input
        .file_name()
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

            let errors = validator::validate_bu_file_direct(
                sk,
                &input.display().to_string(),
                &HashSet::new(),
                &ast::Rank::Skirmish,
            );
            if !errors.is_empty() {
                for e in &errors { eprintln!("error: {}", e); }
                std::process::exit(1);
            }

            let type_errors = typecheck::typecheck_file(
                sk,
                &input.display().to_string(),
            );
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

// ── War root guard ────────────────────────────────────────────────────────────

/// Verify the given path is a valid war root.
/// TODO (global invocation): walk up from cwd to find the war root
/// automatically, like tsc walks up to find tsconfig.json.
fn guard_war_root(root: &PathBuf) {
    if !root.is_dir() {
        eprintln!("error: '{}' is not a directory", root.display());
        std::process::exit(1);
    }

    let inv = root.join("inventory.bu");
    if !inv.exists() {
        eprintln!(
            "error: no inventory.bu found in '{}'\n\
             run bullang from inside the war root directory",
            root.display()
        );
        std::process::exit(1);
    }

    match validator::read_folder_rank(root) {
        Some(ast::Rank::War) => {}
        Some(other) => {
            eprintln!(
                "error: '{}' declares #rank: {} — expected #rank: war\n\
                 run bullang from inside the war root directory",
                root.display(), other.name()
            );
            std::process::exit(1);
        }
        None => {
            eprintln!(
                "error: could not read rank from '{}/inventory.bu'",
                root.display()
            );
            std::process::exit(1);
        }
    }
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
