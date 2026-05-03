//! File-system helpers and direct single-file validation.

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::fs;
use crate::ast::*;
use crate::parser;
use super::ValidationError;

// ── Child callable collection ─────────────────────────────────────────────────

pub fn collect_child_callable(subdirs: &[PathBuf]) -> HashSet<String> {
    let mut names = HashSet::new();
    for subdir in subdirs {
        if let Ok(inv) = read_inventory(subdir) {
            for entry in &inv.entries {
                for func in &entry.functions {
                    names.insert(func.clone());
                }
            }
            names.extend(collect_child_callable(&collect_subdirs(subdir)));
        }
    }
    names
}

// ── Inventory / rank readers ──────────────────────────────────────────────────

pub fn read_inventory(dir: &Path) -> Result<InventoryFile, String> {
    let inv_path = dir.join("inventory.bu");
    let source   = fs::read_to_string(&inv_path)
        .map_err(|_| format!(
            "Missing inventory.bu in '{}' — every Bullang folder must have one.",
            dir.display()
        ))?;
    match parser::parse_file(&source, true) {
        Ok(BuFile::Inventory(inv)) => Ok(inv),
        Ok(_)  => Err(format!("inventory.bu in '{}' parsed as a source file.", dir.display())),
        Err(e) => Err(format!("Parse error in inventory.bu: {}", e)),
    }
}

pub fn read_folder_rank(dir: &Path) -> Option<Rank> {
    read_inventory(dir).ok().map(|inv| inv.rank)
}

// ── Path helpers ──────────────────────────────────────────────────────────────

pub fn main_bu_path(dir: &Path) -> Option<PathBuf> {
    let p = dir.join("main.bu");
    if p.exists() { Some(p) } else { None }
}

pub fn collect_bu_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().map(|x| x == "bu").unwrap_or(false)
                && p.file_name().and_then(|n| n.to_str())
                    .map(|n| n != "inventory.bu" && n != "main.bu" && n != "blueprint.bu")
                    .unwrap_or(false)
        })
        .collect();
    files.sort();
    files
}

pub fn collect_subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

// ── String helper ─────────────────────────────────────────────────────────────

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None    => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// ── Direct single-file validation (for `bullang convert file.bu`) ─────────────

pub fn validate_source_direct(
    sf:       &SourceFile,
    path:     &str,
    callable: &HashSet<String>,
    rank:     &Rank,
) -> Vec<ValidationError> {
    use super::source::validate_function;

    let mut errors  = Vec::new();
    let is_skirmish = rank == &Rank::Skirmish;

    if sf.bullets.len() > 5 {
        errors.push(ValidationError {
            file: path.to_string(), line: 0, col: 0,
            message: format!(
                "A source file cannot contain more than 5 functions (found {}).",
                sf.bullets.len()
            ),
        });
    }

    for func in &sf.bullets {
        errors.extend(validate_function(func, path, callable, is_skirmish));
    }

    errors
}
