//! Tree-walk build pass — rank-agnostic, any rank as root.
//! Handles main.bu at any rank except skirmish.

use std::path::{Path, PathBuf};
use std::fs;

use crate::ast::{BuFile, Rank, Backend};
use crate::codegen;
use crate::parser;
use crate::validator::{
    ValidationError, collect_bu_files, collect_subdirs,
    read_inventory, main_bu_path,
};

pub struct BuildResult {
    pub errors:        Vec<ValidationError>,
    pub files_written: usize,
}

pub fn build(root: &Path, out_dir: &Path, crate_name: &str, backend: &Backend) -> BuildResult {
    let mut errors        = Vec::new();
    let mut files_written = 0;

    let src_out  = out_dir.join("src");
    fs::create_dir_all(&src_out).expect("could not create out/src");

    // Check if any level has a main.bu (for Cargo.toml selection)
    let has_main = tree_has_main(root);

    let (child_modules, _) = emit_folder(
        root, &src_out, backend, crate_name, &mut errors, &mut files_written,
    );

    write_file(&src_out.join("lib.rs"), &codegen::emit_lib_rs(&child_modules), &mut files_written);

    let cargo = if has_main {
        codegen::emit_cargo_toml_with_main(crate_name)
    } else {
        codegen::emit_cargo_toml(crate_name)
    };
    write_file(&out_dir.join("Cargo.toml"), &cargo, &mut files_written);

    BuildResult { errors, files_written }
}

fn emit_folder(
    src_dir:    &Path,
    out_dir:    &Path,
    backend:    &Backend,
    crate_name: &str,
    errors:     &mut Vec<ValidationError>,
    written:    &mut usize,
) -> (Vec<String>, Vec<String>) {
    let inv = match read_inventory(src_dir) {
        Ok(i)  => i,
        Err(_) => return (vec![], vec![]),
    };

    let mut child_modules: Vec<String> = Vec::new();
    let mut all_fns:       Vec<String> = Vec::new();

    // War: only sub-folders (+ optional main.bu)
    if inv.rank == Rank::War {
        for subdir in collect_subdirs(src_dir) {
            let name      = dir_name(&subdir);
            let child_out = out_dir.join(&name);
            fs::create_dir_all(&child_out).ok();
            let (gc, fns) = emit_folder(&subdir, &child_out, backend, crate_name, errors, written);
            write_file(&child_out.join("mod.rs"), &codegen::emit_mod_rs(&gc), written);
            merge(&fns, &mut all_fns);
            child_modules.push(name);
        }
        // Emit main.bu if present at war level
        if let Some(mp) = main_bu_path(src_dir) {
            emit_main_file(&mp, out_dir, crate_name, errors, written);
        }
        return (child_modules, all_fns);
    }

    // Sub-folders first (bottom-up)
    if inv.rank.has_sub_folders() {
        for subdir in collect_subdirs(src_dir) {
            let name      = dir_name(&subdir);
            let child_out = out_dir.join(&name);
            fs::create_dir_all(&child_out).ok();
            let (gc, fns) = emit_folder(&subdir, &child_out, backend, crate_name, errors, written);
            write_file(&child_out.join("mod.rs"), &codegen::emit_mod_rs(&gc), written);
            merge(&fns, &mut all_fns);
            child_modules.push(name);
        }
    }

    // Source files in inventory order
    if inv.rank.has_own_files() {
        for entry in &inv.entries {
            let bu_path = src_dir.join(format!("{}.bu", entry.file));
            let source  = match fs::read_to_string(&bu_path) {
                Ok(s)  => s,
                Err(e) => { errors.push(io_err(&bu_path, e)); continue; }
            };
            let sf = match parser::parse_file(&source, false) {
                Ok(BuFile::Source(s))    => s,
                Ok(BuFile::Inventory(_)) => continue,
                Err(e) => { errors.push(parse_err(&bu_path, e)); continue; }
            };

            merge(&entry.functions, &mut all_fns);
            write_file(
                &out_dir.join(format!("{}.{}", entry.file, backend.ext())),
                &codegen::emit_source(&sf),
                written,
            );
            child_modules.push(entry.file.clone());
        }
    }

    // Emit main.bu if present at this level (not skirmish — validator already caught that)
    if inv.rank != Rank::Skirmish {
        if let Some(mp) = main_bu_path(src_dir) {
            emit_main_file(&mp, out_dir, crate_name, errors, written);
        }
    }

    (child_modules, all_fns)
}

// ── main.bu emitter ───────────────────────────────────────────────────────────

fn emit_main_file(
    main_path:  &Path,
    out_dir:    &Path,
    crate_name: &str,
    errors:     &mut Vec<ValidationError>,
    written:    &mut usize,
) {
    let source = match fs::read_to_string(main_path) {
        Ok(s)  => s,
        Err(e) => { errors.push(io_err(main_path, e)); return; }
    };
    let sf = match parser::parse_file(&source, false) {
        Ok(BuFile::Source(s)) => s,
        Ok(BuFile::Inventory(_)) => return,
        Err(e) => { errors.push(parse_err(main_path, e)); return; }
    };

    write_file(
        &out_dir.join("main.rs"),
        &codegen::emit_main(&sf, crate_name),
        written,
    );
}

// ── Tree scan ─────────────────────────────────────────────────────────────────

/// Returns true if any folder in the tree (except skirmish) has a main.bu.
fn tree_has_main(dir: &Path) -> bool {
    if main_bu_path(dir).is_some() { return true; }
    for subdir in collect_subdirs(dir) {
        if tree_has_main(&subdir) { return true; }
    }
    false
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn write_file(path: &Path, content: &str, written: &mut usize) {
    if let Some(p) = path.parent() { fs::create_dir_all(p).ok(); }
    if fs::write(path, content).is_ok() { *written += 1; }
}

fn dir_name(path: &Path) -> String {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string()
}

fn merge(src: &[String], dst: &mut Vec<String>) {
    for name in src { if !dst.contains(name) { dst.push(name.clone()); } }
}

fn io_err(path: &Path, e: std::io::Error) -> ValidationError {
    ValidationError { file: path.display().to_string(), line: 0, col: 0,
        message: format!("Could not read: {}", e) }
}

fn parse_err(path: &Path, e: Box<dyn std::error::Error>) -> ValidationError {
    ValidationError { file: path.display().to_string(), line: 0, col: 0,
        message: format!("Parse error: {}", e) }
}
