//! Tree-walk build pass.
//!
//! Accepts any rank as root — war, theater, tactic, skirmish, etc.
//! Emits a complete standalone crate into the output directory.
//! The source .bu tree is never modified.

use std::path::{Path, PathBuf};
use std::fs;

use crate::ast::{BuFile, Rank, Backend};
use crate::codegen;
use crate::parser;
use crate::validator::{ValidationError, collect_bu_files, collect_subdirs, read_folder_rank};

// ── Public result ─────────────────────────────────────────────────────────────

pub struct BuildResult {
    pub errors:        Vec<ValidationError>,
    pub files_written: usize,
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn build(
    root:       &Path,
    out_dir:    &Path,
    crate_name: &str,
    backend:    &Backend,
) -> BuildResult {
    let mut errors        = Vec::new();
    let mut files_written = 0;

    let src_out = out_dir.join("src");
    fs::create_dir_all(&src_out).expect("could not create out/src");

    let (child_modules, exports) =
        emit_folder(root, &src_out, backend, &mut errors, &mut files_written);

    write_file(
        &src_out.join("lib.rs"),
        &codegen::emit_lib_rs(&child_modules, &exports),
        &mut files_written,
    );
    write_file(
        &out_dir.join("Cargo.toml"),
        &codegen::emit_cargo_toml(crate_name),
        &mut files_written,
    );

    BuildResult { errors, files_written }
}

// ── Recursive emitter ─────────────────────────────────────────────────────────

/// Emit all files for `src_dir` into `out_dir`.
/// Returns (rust_module_names, exported_bullet_names).
fn emit_folder(
    src_dir: &Path,
    out_dir: &Path,
    backend: &Backend,
    errors:  &mut Vec<ValidationError>,
    written: &mut usize,
) -> (Vec<String>, Vec<String>) {
    let rank = match read_folder_rank(src_dir) {
        Some(r) => r,
        None    => return (vec![], vec![]),
    };

    let mut child_modules: Vec<String> = Vec::new();
    let mut all_exports:   Vec<String> = Vec::new();

    // Recurse into sub-folders first (bottom-up), if this rank has them
    if rank.has_sub_folders() {
        for subdir in collect_subdirs(src_dir) {
            let name      = dir_name(&subdir);
            let child_out = out_dir.join(&name);
            fs::create_dir_all(&child_out).ok();

            let (grandchildren, sub_exports) =
                emit_folder(&subdir, &child_out, backend, errors, written);

            write_file(
                &child_out.join("mod.rs"),
                &codegen::emit_mod_rs(&grandchildren, &sub_exports),
                written,
            );

            merge(&sub_exports, &mut all_exports);
            child_modules.push(name);
        }
    }

    // Emit .bu files at this level, if this rank has them
    if rank.has_own_files() {
        for bu_path in collect_bu_files(src_dir) {
            let stem = file_stem(&bu_path);

            let source = match fs::read_to_string(&bu_path) {
                Ok(s)  => s,
                Err(e) => { errors.push(io_err(&bu_path, e)); continue; }
            };

            let sk = match parser::parse_file(&source, false) {
                Ok(BuFile::Skirmish(s))  => s,
                Ok(BuFile::Inventory(_)) => continue,
                Err(e) => { errors.push(parse_err(&bu_path, e)); continue; }
            };

            merge(&sk.exports, &mut all_exports);
            write_file(
                &out_dir.join(format!("{}.{}", stem, backend.ext())),
                &codegen::emit_skirmish(&sk),
                written,
            );
            child_modules.push(stem);
        }
    }

    (child_modules, all_exports)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn write_file(path: &Path, content: &str, written: &mut usize) {
    if let Some(p) = path.parent() { fs::create_dir_all(p).ok(); }
    if fs::write(path, content).is_ok() { *written += 1; }
}

fn dir_name(path: &Path)  -> String { path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string() }
fn file_stem(path: &Path) -> String { path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string() }

fn merge(src: &[String], dst: &mut Vec<String>) {
    for name in src { if !dst.contains(name) { dst.push(name.clone()); } }
}

fn io_err(path: &Path, e: std::io::Error) -> ValidationError {
    ValidationError { file: path.display().to_string(), line: 0, col: 0,
        message: format!("could not read file: {}", e) }
}

fn parse_err(path: &Path, e: Box<dyn std::error::Error>) -> ValidationError {
    ValidationError { file: path.display().to_string(), line: 0, col: 0,
        message: format!("parse error: {}", e) }
}
