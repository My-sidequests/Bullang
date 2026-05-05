//! Miscellaneous commands: install, update, stdlib, check, lsp.

use crate::{validator, typecheck};
use crate::utils::{current_dir, find_root_from, print_all_errors, print_type_errors};
use crate::stdlib;

/// The canonical source repository. Override with `bullang update --repo <url>`.
pub const DEFAULT_REPO: &str = "https://github.com/My-sidequests/Bullang.git";

// ── install ───────────────────────────────────────────────────────────────────

pub fn cmd_install() {
    println!("Installing bullang via cargo...");
    let status = std::process::Command::new("cargo")
        .args(["install", "--path", "."])
        .status();

    if let Ok(s) = status {
        if s.success() { println!("Installed to ~/.cargo/bin"); }
    }
}

// ── update ────────────────────────────────────────────────────────────────────

pub fn cmd_update(experimental: bool) {
    let branch = if experimental { "experimental" } else { "main" };

    if experimental {
        println!("Updating bullang to the experimental branch...");
        println!("(run `bullang update` without --experimental to revert to main)");
    } else {
        println!("Updating bullang from main branch...");
    }

    let status = std::process::Command::new("cargo")
        .args(["install", "--git", DEFAULT_REPO, "--branch", branch, "bullang"])
        .status();

    if let Ok(s) = status {
        if s.success() {
            println!("Update complete (branch: {}).", branch);
        }
    }
}

// ── stdlib ────────────────────────────────────────────────────────────────────

pub fn cmd_stdlib(_list: bool) {
    println!("Bullang standard library");
    println!("Available in every backend");
    println!();

    println!("  Math");
    println!("  ----");
    let math = ["abs", "pow", "powf", "sqrt", "clamp"];
    let builtins = stdlib::list_builtins();
    for (name, sig, desc) in &builtins {
        if math.contains(name) {
            println!("    builtin::{:<14}  {}  — {}", name, sig, desc);
        }
    }
    println!();
    println!("  String");
    println!("  ------");
    for (name, sig, desc) in &builtins {
        if !math.contains(name) {
            println!("    builtin::{:<14}  {}  — {}", name, sig, desc);
        }
    }
    println!();
    println!("Usage in a source file:");
    println!();
    println!("  let upper(s: String) -> result: String {{");
    println!("      builtin::to_upper");
    println!("  }}");
    println!();
    println!("  let absolute(x: i32) -> result: i32 {{");
    println!("      builtin::abs");
    println!("  }}");
    println!();
    println!("The function's parameters are passed to the builtin.");
    println!("Parameter counts are enforced at build time.");
}

// ── lsp ───────────────────────────────────────────────────────────────────────

pub fn run_lsp() {
    crate::lsp::run();
}

// ── check ─────────────────────────────────────────────────────────────────────

pub fn cmd_check() {
    let root = find_root_from(&current_dir());
    let rank = validator::read_folder_rank(&root).expect("root has no rank");

    println!("bullang check");
    println!("  root : {} ({})", root.display(), rank.name());
    println!();

    let all_errors = validator::validate_tree(&root);
    if !all_errors.is_empty() {
        print_all_errors(&all_errors);
        std::process::exit(1);
    }

    let type_errors = typecheck::typecheck_tree(&root);
    if !type_errors.is_empty() {
        print_type_errors(&type_errors);
        std::process::exit(1);
    }

    // Format check — report drift without writing anything
    let unformatted = crate::cmd::cmd_fmt::check_formatting(&root);
    if !unformatted.is_empty() {
        eprintln!();
        eprintln!("formatting errors — run `bullang fmt` to fix:");
        for path in &unformatted {
            eprintln!("  {}", path.display());
        }
        eprintln!();
        eprintln!("{} file(s) not in canonical format", unformatted.len());
        std::process::exit(1);
    }

    println!("ok -- no errors found");
}
