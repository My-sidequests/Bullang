//! Tree-walk build pass.
//!
//! Reads a validated war tree and emits a complete standalone Rust crate
//! into the output directory. The source .bu tree is never modified.
//!
//! Output layout:
//!   <out>/
//!   ├── Cargo.toml
//!   └── src/
//!       ├── lib.rs
//!       └── <theater>/
//!           ├── mod.rs
//!           ├── <theater_file>.rs
//!           └── <battle>/
//!               ├── mod.rs
//!               └── ...

use std::path::{Path, PathBuf};
use std::fs;

use crate::ast::{BuFile, Rank, Backend};
use crate::codegen;
use crate::parser;
use crate::validator::{
    ValidationError, collect_bu_files, collect_subdirs, read_folder_rank,
    collect_folder_exports,
};

// ── Public result ─────────────────────────────────────────────────────────────

pub struct BuildResult {
    pub errors:        Vec<ValidationError>,
    pub files_written: usize,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Build a full war tree into a standalone crate at `out_dir`.
/// `crate_name` becomes the [package] name in Cargo.toml.
/// `backend`    selects the target language (currently only Rust).
pub fn build(war_root: &Path, out_dir: &Path, crate_name: &str, backend: &Backend) -> BuildResult {
    let mut errors        = Vec::new();
    let mut files_written = 0;

    let src_out = out_dir.join("src");
    fs::create_dir_all(&src_out).expect("could not create out/src");

    let (child_modules, exports) = emit_folder(
        war_root,
        &src_out,
        backend,
        &mut errors,
        &mut files_written,
    );

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

fn emit_folder(
    src_dir:  &Path,
    out_dir:  &Path,
    backend:  &Backend,
    errors:   &mut Vec<ValidationError>,
    written:  &mut usize,
) -> (Vec<String>, Vec<String>) {
    let rank = match read_folder_rank(src_dir) {
        Some(r) => r,
        None    => return (vec![], vec![]),
    };

    let mut child_modules: Vec<String> = Vec::new();
    let mut all_exports:   Vec<String> = Vec::new();

    // War: only sub-folders, no .bu files
    if rank == Rank::War {
        for subdir in collect_subdirs(src_dir) {
            let name      = folder_name(&subdir);
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
        return (child_modules, all_exports);
    }

    // All other ranks: sub-folders first (bottom-up), then .bu files
    if rank != Rank::Skirmish {
        for subdir in collect_subdirs(src_dir) {
            let name      = folder_name(&subdir);
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

    // .bu files at this level
    for bu_path in collect_bu_files(src_dir) {
        let stem = file_stem(&bu_path);

        let source = match fs::read_to_string(&bu_path) {
            Ok(s)  => s,
            Err(e) => { errors.push(io_err(&bu_path, e)); continue; }
        };

        let bu = match parser::parse_file(&source, false) {
            Ok(f)  => f,
            Err(e) => { errors.push(parse_err(&bu_path, e)); continue; }
        };

        if let BuFile::Skirmish(ref sk) = bu {
            merge(&sk.exports, &mut all_exports);

            let ext     = backend.ext();
            let rs_path = out_dir.join(format!("{}.{}", stem, ext));
            write_file(&rs_path, &codegen::emit_skirmish(sk), written);
            child_modules.push(stem);
        }
    }

    (child_modules, all_exports)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn write_file(path: &Path, content: &str, written: &mut usize) {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).ok(); }
    if fs::write(path, content).is_ok() { *written += 1; }
}

fn folder_name(path: &Path) -> String {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string()
}

fn file_stem(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string()
}

fn merge(src: &[String], dst: &mut Vec<String>) {
    for name in src {
        if !dst.contains(name) { dst.push(name.clone()); }
    }
}

fn io_err(path: &Path, e: std::io::Error) -> ValidationError {
    ValidationError {
        file: path.display().to_string(), line: 0, col: 0,
        message: format!("could not read file: {}", e),
    }
}

fn parse_err(path: &Path, e: Box<dyn std::error::Error>) -> ValidationError {
    ValidationError {
        file: path.display().to_string(), line: 0, col: 0,
        message: format!("parse error: {}", e),
    }
}
