//! M2 tree-walk build pass.
//!
//! Reads a war tree bottom-up, validates everything, then emits a complete
//! standalone Rust crate into --out. The source .bu tree is never modified.
//!
//! Output layout:
//!   <out>/
//!   ├── Cargo.toml
//!   └── src/
//!       ├── lib.rs              ← war root
//!       └── <theater>/
//!           ├── mod.rs
//!           ├── <theater_file>.rs
//!           └── <battle>/
//!               ├── mod.rs
//!               ├── <battle_file>.rs
//!               └── ...

use std::collections::HashSet;
use std::path::Path;
use std::fs;

use crate::ast::{BuFile, Rank};
use crate::codegen;
use crate::parser;
use crate::validator::{
    self, ValidationError,
    collect_bu_files, collect_subdirs, collect_folder_exports, read_folder_rank,
};

// ── Public result ─────────────────────────────────────────────────────────────

pub struct BuildResult {
    pub errors:        Vec<ValidationError>,
    pub files_written: usize,
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn build(war_root: &Path, out_dir: &Path) -> BuildResult {
    let mut errors        = Vec::new();
    let mut files_written = 0;

    // 1. Validate the full tree bottom-up before emitting anything
    errors.extend(validator::validate_tree(war_root));
    if !errors.is_empty() {
        return BuildResult { errors, files_written };
    }

    // 2. Set up output directories
    let src_out = out_dir.join("src");
    fs::create_dir_all(&src_out).expect("could not create out/src");

    // 3. Walk and emit — bottom-up, mirroring the source tree
    let (child_modules, exports) = emit_folder(
        war_root,
        &src_out,
        &mut errors,
        &mut files_written,
    );

    // 4. lib.rs — the crate root, mirrors the war inventory
    write_file(
        &src_out.join("lib.rs"),
        &codegen::emit_lib_rs(&child_modules, &exports),
        &mut files_written,
    );

    // 5. Cargo.toml
    let crate_name = folder_name(war_root);
    write_file(
        &out_dir.join("Cargo.toml"),
        &codegen::emit_cargo_toml(&crate_name),
        &mut files_written,
    );

    BuildResult { errors, files_written }
}

// ── Recursive emitter ─────────────────────────────────────────────────────────

/// Emit all Rust files for `src_dir` into `out_dir`.
/// Returns (rust_module_names, exported_bullet_names) for the parent to use.
fn emit_folder(
    src_dir:  &Path,
    out_dir:  &Path,
    errors:   &mut Vec<ValidationError>,
    written:  &mut usize,
) -> (Vec<String>, Vec<String>) {
    let rank = match read_folder_rank(src_dir) {
        Some(r) => r,
        None    => return (vec![], vec![]),
    };

    let mut child_modules: Vec<String> = Vec::new();
    let mut all_exports:   Vec<String> = Vec::new();

    // ── Emit sub-folders first (bottom-up) ────────────────────────────────────
    if rank != Rank::Skirmish && rank != Rank::War {
        // This folder can have both sub-folders and .bu files
        for subdir in collect_subdirs(src_dir) {
            let name      = folder_name(&subdir);
            let child_out = out_dir.join(&name);
            fs::create_dir_all(&child_out).ok();

            let (grandchildren, sub_exports) =
                emit_folder(&subdir, &child_out, errors, written);

            // mod.rs for the sub-folder
            write_file(
                &child_out.join("mod.rs"),
                &codegen::emit_mod_rs(&grandchildren, &sub_exports),
                written,
            );

            merge_exports(&sub_exports, &mut all_exports);
            child_modules.push(name);
        }
    } else if rank == Rank::War {
        // War: only sub-folders, no .bu files
        for subdir in collect_subdirs(src_dir) {
            let name      = folder_name(&subdir);
            let child_out = out_dir.join(&name);
            fs::create_dir_all(&child_out).ok();

            let (grandchildren, sub_exports) =
                emit_folder(&subdir, &child_out, errors, written);

            write_file(
                &child_out.join("mod.rs"),
                &codegen::emit_mod_rs(&grandchildren, &sub_exports),
                written,
            );

            merge_exports(&sub_exports, &mut all_exports);
            child_modules.push(name);
        }
        // War has no .bu files — return here
        return (child_modules, all_exports);
    } else {
        // Skirmish: only .bu files, no sub-folders
        // (sub-folders already validated away, nothing to recurse into)
    }

    // ── Emit .bu files at this folder level ───────────────────────────────────
    // Build inventory context: everything exported by child folders
    let _child_inventory: HashSet<String> = if rank != Rank::Skirmish {
        collect_subdirs(src_dir)
            .iter()
            .flat_map(|d| collect_folder_exports(d))
            .collect()
    } else {
        HashSet::new()
    };

    for bu_path in collect_bu_files(src_dir) {
        let stem = file_stem(&bu_path);

        let source = match fs::read_to_string(&bu_path) {
            Ok(s)  => s,
            Err(e) => {
                errors.push(io_err(&bu_path, e));
                continue;
            }
        };

        let bu = match parser::parse_file(&source, false) {
            Ok(f)  => f,
            Err(e) => {
                errors.push(parse_err(&bu_path, e));
                continue;
            }
        };

        if let BuFile::Skirmish(ref sk) = bu {
            // Collect this file's exports for the parent's mod.rs
            merge_exports(&sk.exports, &mut all_exports);

            // Emit <stem>.rs
            let rs_path = out_dir.join(format!("{}.rs", stem));
            write_file(&rs_path, &codegen::emit_skirmish(sk), written);
            child_modules.push(stem);
        }
    }

    (child_modules, all_exports)
}

// ── File writing ──────────────────────────────────────────────────────────────

fn write_file(path: &Path, content: &str, written: &mut usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if fs::write(path, content).is_ok() {
        *written += 1;
    } else {
        eprintln!("warning: could not write {}", path.display());
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn folder_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn merge_exports(src: &[String], dst: &mut Vec<String>) {
    for name in src {
        if !dst.contains(name) {
            dst.push(name.clone());
        }
    }
}

fn io_err(path: &Path, e: std::io::Error) -> ValidationError {
    ValidationError {
        file:    path.display().to_string(),
        message: format!("could not read file: {}", e),
    }
}

fn parse_err(path: &Path, e: Box<dyn std::error::Error>) -> ValidationError {
    ValidationError {
        file:    path.display().to_string(),
        message: format!("parse error: {}", e),
    }
}
