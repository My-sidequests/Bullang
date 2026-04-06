mod ast;
mod build;
mod codegen;
mod parser;
mod validator;

use clap::{Parser as ClapParser, Subcommand};
use std::path::PathBuf;

#[derive(ClapParser)]
#[command(
    name    = "bullang",
    version = "0.1.0",
    about   = "Bullang (.bu) -> Rust transpiler\n\n\
               The war tree is read-only. All output lands in --out.\n\
               Source and output are always completely separate."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Transpile a single .bu file to stdout (or --output file)
    File {
        /// Path to the .bu source file
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Validate a war tree without emitting any code
    Check {
        /// Path to the war root directory (must contain inventory.bu with #rank: war)
        root: PathBuf,
    },

    /// Validate and transpile a full war tree into a standalone Rust crate
    Build {
        /// Path to the war root directory (must contain inventory.bu with #rank: war)
        root: PathBuf,
        /// Output directory for the generated Rust crate
        #[arg(short, long, default_value = "bullang-out")]
        out: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::File { input, output }  => cmd_file(input, output),
        Command::Check { root }          => cmd_check(root),
        Command::Build { root, out }     => cmd_build(root, out),
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
            // Single-file mode: no cross-file inventory context available.
            // Internal bullet rules are still validated.
            use std::collections::HashSet;
            let errors = validator::validate_bu_file_direct(
                sk,
                &input.display().to_string(),
                &HashSet::new(),
                &ast::Rank::Skirmish,
            );
            abort_on_errors(&errors);
            write_or_print(codegen::emit_skirmish(sk), output);
        }
        ast::BuFile::Inventory(_) => {
            // Inventory in single-file mode — emit an empty mod stub
            write_or_print(codegen::emit_mod_rs(&[], &[]), output);
        }
    }
}

// ── check ─────────────────────────────────────────────────────────────────────

fn cmd_check(root: PathBuf) {
    guard_war_root(&root);
    let errors = validator::validate_tree(&root);
    if errors.is_empty() {
        println!("ok -- {} validated with no errors", root.display());
    } else {
        for e in &errors { eprintln!("error: {}", e); }
        eprintln!("\n{} error(s) found", errors.len());
        std::process::exit(1);
    }
}

// ── build ─────────────────────────────────────────────────────────────────────

fn cmd_build(root: PathBuf, out: PathBuf) {
    guard_war_root(&root);

    // Refuse to write output inside the source tree
    if out.starts_with(&root) || root.starts_with(&out) {
        eprintln!(
            "error: output directory '{}' must be completely outside \
             the war source tree '{}'",
            out.display(), root.display()
        );
        std::process::exit(1);
    }

    println!("bullang build");
    println!("  source : {}", root.display());
    println!("  output : {}", out.display());
    println!();

    let result = build::build(&root, &out);

    for e in &result.errors {
        eprintln!("error: {}", e);
    }

    if !result.errors.is_empty() {
        eprintln!("\nbuild failed -- {} error(s)", result.errors.len());
        std::process::exit(1);
    }

    println!(
        "ok -- {} file(s) written to {}",
        result.files_written,
        out.display()
    );
    println!();
    println!("to compile the generated crate:");
    println!("  cd {} && cargo build", out.display());
}

// ── guards ────────────────────────────────────────────────────────────────────

/// Verify the given path is a valid war root (has inventory.bu with #rank: war).
fn guard_war_root(root: &PathBuf) {
    if !root.is_dir() {
        eprintln!("error: '{}' is not a directory", root.display());
        std::process::exit(1);
    }

    let inv = root.join("inventory.bu");
    if !inv.exists() {
        eprintln!(
            "error: '{}' has no inventory.bu — is this a war root?",
            root.display()
        );
        std::process::exit(1);
    }

    match validator::read_folder_rank(root) {
        Some(ast::Rank::War) => {}
        Some(other) => {
            eprintln!(
                "error: '{}' inventory.bu declares #rank: {} — expected #rank: war",
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

// ── utilities ─────────────────────────────────────────────────────────────────

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

fn abort_on_errors(errors: &[validator::ValidationError]) {
    if !errors.is_empty() {
        for e in errors { eprintln!("error: {}", e); }
        std::process::exit(1);
    }
}
