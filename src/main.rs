mod ast;
mod parser;
mod validator;
mod codegen;

use clap::{Parser as ClapParser, Subcommand};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(ClapParser)]
#[command(
    name = "bullang", 
    version ="0.1.0",
    about = "Bullang (.bu) → Rust transpiler"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate and transpile a full war directory tree
    Build { root: PathBuf },
    /// Validate only, no code emission
    Check { root: PathBuf },
    /// Transpile a single skirmish .bu file
    File {
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::File { input, output } => cmd_file(input, output),
        Command::Check { root }         => cmd_check(root),
        Command::Build { root }         => cmd_build(root),
    }
}

// ── Single file mode ──────────────────────────────────────────────────────────

fn cmd_file(input: PathBuf, output: Option<PathBuf>) {
    let source = read_file(&input);
    let is_inv = input.file_name()
        .map(|n| n == "inventory.bu")
        .unwrap_or(false);

    let bu = parser::parse_file(&source, is_inv).unwrap_or_else(|e| {
        eprintln!("parse error: {}", e);
        std::process::exit(1);
    });

    match bu {
        ast::BuFile::Skirmish(ref skirmish) => {
            // In single-file mode we have no inventory context — use empty set
            let inventory: HashSet<String> = HashSet::new();
            let errors = validator::validate_skirmish(
                skirmish,
                &input.display().to_string(),
                &inventory,
            );
            abort_on_errors(&errors);

            let rust = codegen::emit_skirmish(skirmish);
            write_or_print(rust, output);
        }
        ast::BuFile::Inventory(ref inv) => {
            let rust = codegen::emit_inventory(inv, &[]);
            write_or_print(rust, output);
        }
    }
}

// ── Check mode ────────────────────────────────────────────────────────────────

fn cmd_check(root: PathBuf) {
    let errors = run_tree_validation(&root);
    if errors.is_empty() {
        println!("ok — no errors found");
    } else {
        for e in &errors { eprintln!("error: {}", e); }
        std::process::exit(1);
    }
}

// ── Build mode ────────────────────────────────────────────────────────────────

fn cmd_build(root: PathBuf) {
    let errors = run_tree_validation(&root);
    abort_on_errors(&errors);
    println!("validation passed");
    // TODO: walk tree, emit each .bu → .rs, each inventory → mod.rs
    // This will be implemented in the tree-walk build pass
}

// ── Tree validation ───────────────────────────────────────────────────────────

fn run_tree_validation(root: &PathBuf) -> Vec<validator::ValidationError> {
    validator::validate_folder_counts(root, &ast::Rank::War, root)
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn read_file(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error reading {:?}: {}", path, e);
        std::process::exit(1);
    })
}

fn write_or_print(content: String, output: Option<PathBuf>) {
    match output {
        Some(p) => std::fs::write(&p, &content).unwrap_or_else(|e| {
            eprintln!("error writing {:?}: {}", p, e);
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
