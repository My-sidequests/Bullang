//! Bullang compiler entry point.
//!
//! Global invocation (like tsc/go): install once, run from anywhere.
//!
//!   bullang install                        — install to system PATH
//!   bullang convert my_folder              — transpile (default: _my_folder, rs)
//!   bullang convert my_folder -n out_name  — custom output name
//!   bullang convert my_folder -e rs        — explicit extension
//!   bullang convert my_folder --out /path  — explicit output path
//!   bullang check                          — validate from cwd (walks up to root)
//!   bullang file path/to/file.bu           — single file, no tree context

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
               Install once with `bullang install`, then run from anywhere.\n\
               The source tree is never modified — all output is external."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Install bullang to your system PATH so it can be run from anywhere.
    Install,

    /// Transpile a Bullang project folder into a target-language crate/project.
    ///
    /// Examples:
    ///   bullang convert my_folder
    ///   bullang convert my_folder -n my_lib -e rs
    ///   bullang convert my_folder --out ~/projects/my_lib
    Convert {
        /// Path to the Bullang source folder.
        /// Defaults to the current directory.
        folder: Option<PathBuf>,

        /// Output folder name (placed next to the source folder).
        /// Default: _<source_folder_name>
        #[arg(short = 'n', long)]
        name: Option<String>,

        /// Target language extension (default: rs).
        /// Supported: rs
        #[arg(short = 'e', long, default_value = "rs")]
        ext: String,

        /// Explicit full output path. Overrides -n when given.
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Validate and type-check the project from the current directory.
    /// Walks up automatically to find the root, like tsc.
    Check,

    /// Transpile a single .bu file to stdout or --output.
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
        Command::Install                             => cmd_install(),
        Command::Convert { folder, name, ext, out } => cmd_convert(folder, name, ext, out),
        Command::Check                              => cmd_check(),
        Command::File { input, output }             => cmd_file(input, output),
    }
}

// ── install ───────────────────────────────────────────────────────────────────

fn cmd_install() {
    // The currently running binary is the one we want to install.
    let current_exe = std::env::current_exe().unwrap_or_else(|e| {
        eprintln!("error: could not locate current binary: {}", e);
        std::process::exit(1);
    });

    // Try system-wide first, fall back to user-local
    let candidates: &[&str] = &[
        "/usr/local/bin/bullang",
        "/usr/bin/bullang",
    ];

    let user_local = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/.local/bin/bullang", home)
    };

    let dest = find_install_dest(candidates, &user_local);

    // Ensure the destination directory exists
    if let Some(parent) = Path::new(&dest).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("error: could not create {}: {}", parent.display(), e);
            std::process::exit(1);
        }
    }

    // Copy the binary
    match std::fs::copy(&current_exe, &dest) {
        Ok(_) => {
            // Make executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest)
                    .unwrap().permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest, perms).ok();
            }
            println!("installed: {}", dest);
            println!();
            println!("bullang is now available globally.");
            println!("you can run `bullang convert <folder>` from anywhere.");

            // Warn if the install dir is not in PATH
            check_path_contains(&dest);
        }
        Err(e) => {
            eprintln!("error: could not install to {}: {}", dest, e);
            eprintln!();
            eprintln!("try running with sudo, or install to a user-local path:");
            eprintln!("  sudo bullang install");
            std::process::exit(1);
        }
    }
}

/// Find the first writable install destination.
fn find_install_dest(system_paths: &[&str], user_fallback: &str) -> String {
    for path in system_paths {
        let dir = Path::new(path).parent().unwrap_or(Path::new("/usr/local/bin"));
        if is_writable(dir) {
            return path.to_string();
        }
    }
    user_fallback.to_string()
}

fn is_writable(path: &Path) -> bool {
    // Try creating a temp file in the directory
    if !path.exists() { return false; }
    let test = path.join(".bullang_write_test");
    match std::fs::write(&test, b"") {
        Ok(_) => { std::fs::remove_file(test).ok(); true }
        Err(_) => false,
    }
}

fn check_path_contains(dest: &str) {
    let dest_dir = Path::new(dest).parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let path_var = std::env::var("PATH").unwrap_or_default();
    let in_path  = path_var.split(':').any(|p| p == dest_dir);

    if !in_path {
        println!();
        println!("note: {} is not in your PATH.", dest_dir);
        println!("add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):");
        println!("  export PATH=\"{}:$PATH\"", dest_dir);
    }
}

// ── convert ───────────────────────────────────────────────────────────────────

fn cmd_convert(
    folder: Option<PathBuf>,
    name:   Option<String>,
    ext:    String,
    out:    Option<PathBuf>,
) {
    // Resolve the backend
    let backend = Backend::from_ext(&ext).unwrap_or_else(|| {
        eprintln!(
            "error: unknown extension '{}'\n\
             supported: rs",
            ext
        );
        std::process::exit(1);
    });

    // Resolve the source folder
    let source_dir = match folder {
        Some(ref p) => {
            let canonical = p.canonicalize().unwrap_or_else(|_| p.clone());
            if !canonical.is_dir() {
                eprintln!("error: '{}' is not a directory", p.display());
                std::process::exit(1);
            }
            canonical
        }
        None => std::env::current_dir().unwrap_or_else(|e| {
            eprintln!("error: could not determine current directory: {}", e);
            std::process::exit(1);
        }),
    };

    // Find the Bullang root (walk up from source_dir)
    let root = find_root_from(&source_dir);

    // Derive the source folder name for default output naming
    let source_name = source_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("bullang_project")
        .to_string();

    // Resolve the output directory
    // Priority: --out > -n > default (_<source_name>)
    let out_dir = match out {
        Some(p) => p,
        None => {
            let out_name = name.unwrap_or_else(|| format!("_{}", source_name));
            // Place next to the source folder
            source_dir
                .parent()
                .unwrap_or(&source_dir)
                .join(out_name)
        }
    };

    // Refuse to write inside the source tree
    if out_dir.starts_with(&root) || root.starts_with(&out_dir) {
        eprintln!(
            "error: output '{}' must be outside the source tree '{}'",
            out_dir.display(), root.display()
        );
        std::process::exit(1);
    }

    // Derive crate name from the output folder name
    let crate_name = out_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("bullang_out")
        .to_string();

    let root_rank = validator::read_folder_rank(&root)
        .expect("root inventory.bu has no rank");

    println!("bullang convert");
    println!("  source  : {} ({})", root.display(), root_rank.name());
    println!("  output  : {}", out_dir.display());
    println!("  crate   : {}", crate_name);
    println!("  backend : {}", backend.name());
    println!();

    // Phase 1: structural validation
    let errors = validator::validate_tree(&root);
    if !errors.is_empty() {
        for e in &errors { eprintln!("error: {}", e); }
        eprintln!("\n{} structural error(s) — convert aborted", errors.len());
        std::process::exit(1);
    }
    println!("structural validation ... ok");

    // Phase 2: type checking
    let type_errors = typecheck::typecheck_tree(&root);
    if !type_errors.is_empty() {
        for e in &type_errors { eprintln!("type error: {}", e); }
        eprintln!("\n{} type error(s) — convert aborted", type_errors.len());
        std::process::exit(1);
    }
    println!("type checking         ... ok");

    // Phase 3: code generation
    let result = build::build(&root, &out_dir, &crate_name, &backend);
    for e in &result.errors { eprintln!("error: {}", e); }
    if !result.errors.is_empty() {
        eprintln!("\nconvert failed -- {} error(s)", result.errors.len());
        std::process::exit(1);
    }

    println!("code generation       ... ok");
    println!();
    println!("wrote {} file(s) to {}", result.files_written, out_dir.display());
    println!();

    match backend {
        Backend::Rust => {
            println!("to compile the generated Rust crate:");
            println!("  cd {} && cargo build", out_dir.display());
        }
    }
}

// ── check ─────────────────────────────────────────────────────────────────────

fn cmd_check() {
    let cwd  = current_dir();
    let root = find_root_from(&cwd);
    let rank = validator::read_folder_rank(&root).expect("root has no rank");

    println!("bullang check");
    println!("  root : {} ({})", root.display(), rank.name());
    println!();

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

/// Walk UP from `start` to find the topmost Bullang root.
///
/// The root is the highest ancestor that still contains an inventory.bu.
/// This allows `bullang convert` and `bullang check` to work from any
/// subdirectory inside a project, just like `tsc` with tsconfig.json.
fn find_root_from(start: &Path) -> PathBuf {
    if !start.join("inventory.bu").exists() {
        eprintln!(
            "error: no inventory.bu found in '{}'\n\
             '{}' does not appear to be a Bullang project folder",
            start.display(), start.display()
        );
        std::process::exit(1);
    }

    let mut root = start.to_path_buf();
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

    if validator::read_folder_rank(&root).is_none() {
        eprintln!(
            "error: found inventory.bu at '{}' but could not read its #rank",
            root.display()
        );
        std::process::exit(1);
    }

    root
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("error: could not determine current directory: {}", e);
        std::process::exit(1);
    })
}

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
