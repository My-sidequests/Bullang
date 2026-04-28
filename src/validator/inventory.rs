//! Inventory completeness checks: every file and function must be declared.

use std::path::{Path, PathBuf};
use std::collections::{HashSet, HashMap};
use std::fs;
use crate::ast::*;
use crate::parser;
use super::ValidationError;

pub fn build_inv_map(inv: &InventoryFile) -> HashMap<String, Vec<String>> {
    inv.entries.iter()
        .map(|e| (e.file.clone(), e.functions.clone()))
        .collect()
}

pub fn validate_inventory_completeness(
    dir:      &Path,
    inv:      &InventoryFile,
    bu_files: &[PathBuf],
    _subdirs: &[PathBuf],
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let inv_path   = dir.join("inventory.bu");
    let inv_str    = inv_path.display().to_string();

    let file_stems: HashSet<String> = bu_files.iter()
        .filter_map(|p| p.file_stem()?.to_str().map(|s| s.to_string()))
        .collect();

    let inv_stems: HashSet<String> = inv.entries.iter()
        .map(|e| e.file.clone())
        .collect();

    for stem in &file_stems {
        if !inv_stems.contains(stem) {
            errors.push(ferr(&inv_str, format!(
                "Source file '{}.bu' exists but is not listed in inventory. \
                 Add a line:  {}: fn1, fn2, ...;",
                stem, stem
            )));
        }
    }

    for stem in &inv_stems {
        if !file_stems.contains(stem) {
            errors.push(ferr(&inv_str, format!(
                "Inventory lists '{}' but '{}.bu' does not exist in this folder.",
                stem, stem
            )));
        }
    }

    for entry in &inv.entries {
        if !file_stems.contains(&entry.file) { continue; }

        let bu_path = dir.join(format!("{}.bu", entry.file));
        let source  = match fs::read_to_string(&bu_path) {
            Ok(s)  => s,
            Err(_) => continue,
        };

        let sf = match parser::parse_file(&source, false) {
            Ok(BuFile::Source(s)) => s,
            _ => continue,
        };

        let actual_fns: HashSet<&str> = sf.bullets.iter()
            .map(|b| b.name.as_str()).collect();
        let listed_fns: HashSet<&str> = entry.functions.iter()
            .map(|f| f.as_str()).collect();

        for name in &actual_fns {
            if !listed_fns.contains(name) {
                errors.push(ferr(&inv_str, format!(
                    "Function '{}' exists in '{}.bu' but is not listed in inventory.",
                    name, entry.file
                )));
            }
        }

        for name in &listed_fns {
            if !actual_fns.contains(name) {
                errors.push(ferr(&inv_str, format!(
                    "The function '{}' is listed in inventory, but not found in '{}.bu'.",
                    name, entry.file
                )));
            }
        }
    }

    errors
}

// ── Local error constructor ───────────────────────────────────────────────────

fn ferr(file: &str, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: file.to_string(), line: 0, col: 0, message: msg.into() }
}
