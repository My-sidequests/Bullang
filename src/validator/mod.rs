//! Compile-time structural validation.
//!
//! Uses tolerant parsing: one broken function does not abort validation
//! of the rest of the file. All errors across all files are collected
//! before returning, so the developer sees the full picture in one run.

mod helpers;
mod inventory;
pub mod source;

pub use helpers::{
    read_inventory, read_folder_rank, main_bu_path,
    collect_bu_files, collect_subdirs, collect_child_callable,
    validate_source_direct,
};

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::fs;
use crate::ast::*;
use crate::parser;

// ── Error types ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ValidationError {
    pub file:    String,
    pub line:    usize,
    pub col:     usize,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(f, "[{}:{}:{}] {}", self.file, self.line, self.col, self.message)
        } else {
            write!(f, "[{}] {}", self.file, self.message)
        }
    }
}

/// All errors from one validation run — parse errors and structural errors
/// kept together so they can be sorted and displayed uniformly.
#[derive(Debug)]
pub struct AllErrors {
    pub parse:      Vec<crate::parser::ParseError>,
    pub structural: Vec<ValidationError>,
}

impl AllErrors {
    pub fn new() -> Self { Self { parse: vec![], structural: vec![] } }
    pub fn is_empty(&self) -> bool { self.parse.is_empty() && self.structural.is_empty() }
    pub fn push_structural(&mut self, e: ValidationError) { self.structural.push(e); }
    pub fn extend_structural(&mut self, es: Vec<ValidationError>) { self.structural.extend(es); }
    pub fn extend_parse(&mut self, es: Vec<crate::parser::ParseError>) { self.parse.extend(es); }
    pub fn extend_all(&mut self, other: AllErrors) {
        self.parse.extend(other.parse);
        self.structural.extend(other.structural);
    }
}

// ── Error constructors ────────────────────────────────────────────────────────

pub(crate) fn err(path: &Path, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: path.display().to_string(), line: 0, col: 0, message: msg.into() }
}

fn serr(file: &str, span: Span, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: file.to_string(), line: span.line, col: span.col, message: msg.into() }
}

fn ferr(file: &str, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: file.to_string(), line: 0, col: 0, message: msg.into() }
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn validate_tree(root: &Path) -> AllErrors {
    validate_folder(root)
}

// ── Folder validation (recursive, bottom-up) ─────────────────────────────────

fn validate_folder(dir: &Path) -> AllErrors {
    let mut all = AllErrors::new();

    let inv = match helpers::read_inventory(dir) {
        Ok(i)  => i,
        Err(e) => {
            all.push_structural(err(dir, e));
            return all;
        }
    };

    let subdirs   = helpers::collect_subdirs(dir);
    let bu_files  = helpers::collect_bu_files(dir);
    let main_path = helpers::main_bu_path(dir);

    // Recurse into sub-folders first (bottom-up)
    for subdir in &subdirs {
        all.extend_all(validate_folder(subdir));
    }

    match inv.rank {
        // ── War ───────────────────────────────────────────────────────────────
        Rank::War => {
            if !bu_files.is_empty() {
                all.push_structural(err(dir, format!(
                    "War folder cannot contain source files (found {}). \
                     Consider using a theater rank instead.",
                    bu_files.len()
                )));
            }
            if subdirs.len() > 5 {
                all.push_structural(err(dir, format!(
                    "War folder cannot exceed 5 theaters (found {}).",
                    subdirs.len()
                )));
            }
            if !inv.entries.is_empty() {
                all.push_structural(err(
                    &dir.join("inventory.bu"),
                    "War inventory cannot list any files."
                ));
            }
            for subdir in &subdirs {
                validate_child_rank(subdir, &Rank::Theater, &mut all);
            }
            if let Some(ref mp) = main_path {
                let child_callable = helpers::collect_child_callable(&subdirs);
                all.extend_all(validate_main_file(mp, &child_callable));
            }
        }

        // ── Skirmish ──────────────────────────────────────────────────────────
        Rank::Skirmish => {
            if !subdirs.is_empty() {
                all.push_structural(err(dir, format!(
                    "Skirmish folder cannot contain sub-folders (found {}).",
                    subdirs.len()
                )));
            }
            if bu_files.len() > 5 {
                all.push_structural(err(dir, format!(
                    "Skirmish folder cannot contain more than 5 source files (found {}).",
                    bu_files.len()
                )));
            }
            if main_path.is_some() {
                all.push_structural(err(
                    &dir.join("main.bu"),
                    "Skirmish folders cannot contain main.bu. \
                     Move your entry point to a tactic or higher rank folder."
                ));
            }
            all.extend_structural(inventory::validate_inventory_completeness(
                dir, &inv, &bu_files, &[],
            ));
            let inv_map = inventory::build_inv_map(&inv);
            for bu in &bu_files {
                all.extend_all(source::validate_source_file(
                    bu, &inv.rank, &inv_map, &HashSet::new(),
                ));
            }
        }

        // ── Middle ranks ──────────────────────────────────────────────────────
        ref rank => {
            let child_rank = rank.child_rank().unwrap();

            if subdirs.len() > 5 {
                all.push_structural(err(dir, format!(
                    "{} folder cannot contain more than 5 {} sub-folders (found {}).",
                    helpers::capitalize(rank.name()), child_rank.name(), subdirs.len()
                )));
            }
            if bu_files.len() > 5 {
                all.push_structural(err(dir, format!(
                    "{} folder cannot contain more than 5 source files (found {}).",
                    helpers::capitalize(rank.name()), bu_files.len()
                )));
            }
            for subdir in &subdirs {
                validate_child_rank(subdir, &child_rank, &mut all);
            }
            all.extend_structural(inventory::validate_inventory_completeness(
                dir, &inv, &bu_files, &subdirs,
            ));
            let child_callable = helpers::collect_child_callable(&subdirs);
            let inv_map        = inventory::build_inv_map(&inv);
            for bu in &bu_files {
                all.extend_all(source::validate_source_file(bu, rank, &inv_map, &child_callable));
            }
            if let Some(ref mp) = main_path {
                all.extend_all(validate_main_file(mp, &child_callable));
            }
        }
    }

    all
}

fn validate_child_rank(subdir: &Path, expected: &Rank, all: &mut AllErrors) {
    match helpers::read_folder_rank(subdir) {
        Some(ref actual) if actual == expected => {}
        Some(ref actual) => {
            all.push_structural(err(subdir, format!(
                "Found unexpected '{}' in inventory. Consider replacing it with '{}'.",
                actual.name(), expected.name()
            )));
        }
        None => {
            all.push_structural(err(subdir, format!(
                "Sub-folder '{}' is missing inventory.bu (expected a {} folder).",
                subdir.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                expected.name()
            )));
        }
    }
}

// ── main.bu validation ────────────────────────────────────────────────────────

fn validate_main_file(path: &Path, callable: &HashSet<String>) -> AllErrors {
    let mut all = AllErrors::new();

    let src = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => {
            all.push_structural(err(path, format!("Could not read main.bu: {}", e)));
            return all;
        }
    };

    let path_str = path.display().to_string();
    let result   = parser::parse_file_tolerant(&src, &path_str);
    all.extend_parse(result.errors);

    if let BuFile::Source(ref sf) = result.file {
        if sf.bullets.len() > 5 {
            all.push_structural(ferr(&path_str, format!(
                "main.bu cannot contain more than 5 functions (found {}).",
                sf.bullets.len()
            )));
        }
        for func in &sf.bullets {
            all.extend_structural(source::validate_function(func, &path_str, callable, false));
        }
    }

    all
}
