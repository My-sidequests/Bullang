//! Compile-time structural validation.
//!
//! Reserved filenames (excluded from inventory): inventory.bu, main.bu
//! main.bu is allowed at any rank except skirmish.

use std::path::{Path, PathBuf};
use std::collections::{HashSet, HashMap};
use std::fs;
use crate::ast::*;
use crate::parser;

// ── Error type ────────────────────────────────────────────────────────────────

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

fn err(path: &Path, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: path.display().to_string(), line: 0, col: 0, message: msg.into() }
}

fn serr(file: &str, span: Span, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: file.to_string(), line: span.line, col: span.col, message: msg.into() }
}

fn ferr(file: &str, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: file.to_string(), line: 0, col: 0, message: msg.into() }
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn validate_tree(root: &Path) -> Vec<ValidationError> {
    validate_folder(root)
}

// ── Folder validation (recursive, bottom-up) ─────────────────────────────────

fn validate_folder(dir: &Path) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let inv = match read_inventory(dir) {
        Ok(i)  => i,
        Err(e) => { errors.push(err(dir, e)); return errors; }
    };

    let subdirs  = collect_subdirs(dir);
    let bu_files = collect_bu_files(dir);    // excludes inventory.bu AND main.bu
    let main_path = main_bu_path(dir);

    // Recurse into sub-folders first (bottom-up)
    for subdir in &subdirs {
        errors.extend(validate_folder(subdir));
    }

    match inv.rank {
        // ── War: sub-folders only + optional main.bu ──────────────────────────
        Rank::War => {
            if !bu_files.is_empty() {
                errors.push(err(dir, format!(
                    "War folder cannot contain source files (found {}). \
                     Consider using a theater rank instead.",
                    bu_files.len()
                )));
            }
            if subdirs.len() > 5 {
                errors.push(err(dir, format!(
                    "War folder cannot exceed 5 theaters (found {}).",
                    subdirs.len()
                )));
            }
            if !inv.entries.is_empty() {
                errors.push(err(
                    &dir.join("inventory.bu"),
                    "War inventory cannot list any files."
                ));
            }
            for subdir in &subdirs {
                validate_child_rank(subdir, &Rank::Theater, &mut errors);
            }
            // Validate main.bu if present
            if let Some(ref mp) = main_path {
                let child_callable = collect_child_callable(&subdirs);
                errors.extend(validate_main_file(mp, &child_callable));
            }
        }

        // ── Skirmish: source files only, no main.bu allowed ───────────────────
        Rank::Skirmish => {
            if !subdirs.is_empty() {
                errors.push(err(dir, format!(
                    "Skirmish folder cannot contain sub-folders (found {}).",
                    subdirs.len()
                )));
            }
            if bu_files.len() > 5 {
                errors.push(err(dir, format!(
                    "Skirmish folder cannot contain more than 5 source files (found {}).",
                    bu_files.len()
                )));
            }
            if main_path.is_some() {
                errors.push(err(
                    &dir.join("main.bu"),
                    "Skirmish folders cannot contain main.bu. \
                     Move your entry point to a tactic or higher rank folder."
                ));
            }
            errors.extend(validate_inventory_completeness(dir, &inv, &bu_files, &[]));
            let inv_map = build_inv_map(&inv);
            for bu in &bu_files {
                errors.extend(validate_source_file(bu, &inv.rank, &inv_map, &HashSet::new()));
            }
        }

        // ── Middle ranks: sub-folders + source files + optional main.bu ───────
        ref rank => {
            let child_rank = rank.child_rank().unwrap();

            if subdirs.len() > 5 {
                errors.push(err(dir, format!(
                    "{} folder cannot contain more than 5 {} sub-folders (found {}).",
                    capitalize(rank.name()), child_rank.name(), subdirs.len()
                )));
            }
            if bu_files.len() > 5 {
                errors.push(err(dir, format!(
                    "{} folder cannot contain more than 5 source files (found {}).",
                    capitalize(rank.name()), bu_files.len()
                )));
            }
            for subdir in &subdirs {
                validate_child_rank(subdir, &child_rank, &mut errors);
            }
            errors.extend(validate_inventory_completeness(dir, &inv, &bu_files, &subdirs));

            let child_callable = collect_child_callable(&subdirs);
            let inv_map = build_inv_map(&inv);
            for bu in &bu_files {
                errors.extend(validate_source_file(bu, rank, &inv_map, &child_callable));
            }
            // Validate main.bu if present
            if let Some(ref mp) = main_path {
                errors.extend(validate_main_file(mp, &child_callable));
            }
        }
    }

    errors
}

fn validate_child_rank(subdir: &Path, expected: &Rank, errors: &mut Vec<ValidationError>) {
    match read_folder_rank(subdir) {
        Some(ref actual) if actual == expected => {}
        Some(ref actual) => {
            errors.push(err(subdir, format!(
                "Found unexpected '{}' in inventory. Consider replacing it with '{}'.",
                actual.name(), expected.name()
            )));
        }
        None => {
            errors.push(err(subdir, format!(
                "Sub-folder '{}' is missing inventory.bu (expected a {} folder).",
                subdir.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                expected.name()
            )));
        }
    }
}

// ── main.bu validation ────────────────────────────────────────────────────────

/// Validate main.bu — same rules as a source file, but:
///   - the `main` function may have no params and return ()
///   - not subject to inventory listing
fn validate_main_file(
    path:     &Path,
    callable: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => { errors.push(err(path, format!("Could not read main.bu: {}", e))); return errors; }
    };

    let sf = match parser::parse_file(&source, false) {
        Ok(BuFile::Source(s))    => s,
        Ok(BuFile::Inventory(_)) => return errors,
        Err(e) => { errors.push(err(path, format!("Parse error in main.bu: {}", e))); return errors; }
    };

    let path_str = path.display().to_string();

    if sf.bullets.len() > 5 {
        errors.push(ferr(&path_str, format!(
            "main.bu cannot contain more than 5 functions (found {}).",
            sf.bullets.len()
        )));
    }

    // main.bu can call anything in the child callable set — it is never skirmish-restricted
    for func in &sf.bullets {
        errors.extend(validate_function(func, &path_str, callable, false));
    }

    errors
}

// ── Inventory completeness validation ────────────────────────────────────────

fn build_inv_map(inv: &InventoryFile) -> HashMap<String, Vec<String>> {
    inv.entries.iter()
        .map(|e| (e.file.clone(), e.functions.clone()))
        .collect()
}

fn validate_inventory_completeness(
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

// ── Source file validation ────────────────────────────────────────────────────

fn validate_source_file(
    path:           &Path,
    folder_rank:    &Rank,
    _inv_map:       &HashMap<String, Vec<String>>,
    child_callable: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => { errors.push(err(path, format!("Could not read file: {}", e))); return errors; }
    };

    let sf = match parser::parse_file(&source, false) {
        Ok(BuFile::Source(s))    => s,
        Ok(BuFile::Inventory(_)) => return errors,
        Err(e) => { errors.push(err(path, format!("Parse error: {}", e))); return errors; }
    };

    let path_str    = path.display().to_string();
    let is_skirmish = folder_rank == &Rank::Skirmish;

    if sf.bullets.len() > 5 {
        errors.push(ferr(&path_str, format!(
            "A source file cannot contain more than 5 functions (found {}).",
            sf.bullets.len()
        )));
    }

    for func in &sf.bullets {
        errors.extend(validate_function(func, &path_str, child_callable, is_skirmish));
    }

    errors
}

// ── Function / bullet validation ──────────────────────────────────────────────

fn validate_function(
    func:        &Bullet,
    path:        &str,
    callable:    &HashSet<String>,
    is_skirmish: bool,
) -> Vec<ValidationError> {
    match &func.body {
        BulletBody::Native { .. } => vec![],
        BulletBody::Pipes(pipes)  => validate_bullets(
            pipes, &func.name, &func.output.name,
            &func.params, path, callable, is_skirmish,
        ),
    }
}

fn validate_bullets(
    bullets:     &[Pipe],
    func_name:   &str,
    output_name: &str,
    params:      &[Param],
    path:        &str,
    callable:    &HashSet<String>,
    is_skirmish: bool,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if bullets.len() > 5 {
        errors.push(ferr(path, format!(
            "Function '{}': cannot contain more than 5 bullets (found {}).",
            func_name, bullets.len()
        )));
    }

    let param_names: HashSet<&str> = params.iter().map(|p| p.name.as_str()).collect();
    let mut bound:    HashSet<String> = HashSet::new();
    let mut consumed: HashSet<String> = HashSet::new();
    let last = bullets.len().saturating_sub(1);

    for (i, bullet) in bullets.iter().enumerate() {
        for input in &bullet.inputs {
            if param_names.contains(input.as_str()) {
                consumed.insert(input.clone());
            } else if bound.contains(input.as_str()) {
                consumed.insert(input.clone());
            } else {
                errors.push(serr(path, bullet.span, format!(
                    "Function '{}' bullet {}: '{}' is an unknown parameter.",
                    func_name, i + 1, input
                )));
            }
        }

        collect_call_errors(
            &bullet.expr, func_name, path, bullet.span,
            callable, is_skirmish, &mut errors,
        );

        if bound.contains(&bullet.binding) {
            errors.push(serr(path, bullet.span, format!(
                "Function '{}': '{{{}}}' is assigned more than once.",
                func_name, bullet.binding
            )));
        }

        if i == last && bullet.binding != output_name {
            errors.push(serr(path, bullet.span, format!(
                "Function '{}': last bullet output '{{{}}}' must match function output '{{{}}}'.",
                func_name, bullet.binding, output_name
            )));
        }

        bound.insert(bullet.binding.clone());
    }

    for b in &bound {
        if b != output_name && !consumed.contains(b) {
            errors.push(ferr(path, format!(
                "Function '{}': '{{{}}}' is produced but never used.",
                func_name, b
            )));
        }
    }

    errors
}

fn collect_call_errors(
    expr:        &Expr,
    func_name:   &str,
    path:        &str,
    span:        Span,
    callable:    &HashSet<String>,
    is_skirmish: bool,
    errors:      &mut Vec<ValidationError>,
) {
    match expr {
        Expr::Atom(a)      => check_atom(a, func_name, path, span, callable, is_skirmish, errors),
        Expr::BinOp(b)     => {
            check_atom(&b.lhs, func_name, path, span, callable, is_skirmish, errors);
            check_atom(&b.rhs, func_name, path, span, callable, is_skirmish, errors);
        }
        Expr::Tuple(exprs) => {
            for e in exprs {
                collect_call_errors(e, func_name, path, span, callable, is_skirmish, errors);
            }
        }
    }
}

fn check_atom(
    atom:        &Atom,
    func_name:   &str,
    path:        &str,
    span:        Span,
    callable:    &HashSet<String>,
    is_skirmish: bool,
    errors:      &mut Vec<ValidationError>,
) {
    if let Atom::Call { name, args } = atom {
        if is_skirmish {
            errors.push(serr(path, span, format!(
                "Function '{}': skirmish files cannot call other functions (found call to '{}').",
                func_name, name
            )));
            return;
        }
        if !callable.is_empty() && !callable.contains(name.as_str()) {
            errors.push(serr(path, span, format!(
                "Function '{}': calls '{}' which is not listed in any child inventory.",
                func_name, name
            )));
        }
        for arg in args {
            if let CallArg::BulletRef(r) = arg {
                if !callable.is_empty() && !callable.contains(r.as_str()) {
                    errors.push(serr(path, span, format!(
                        "Function '{}': references '&{}' which is not listed in any child inventory.",
                        func_name, r
                    )));
                }
            }
        }
    }
}

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

// ── Helpers ───────────────────────────────────────────────────────────────────

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

/// Returns the path to main.bu in this directory, if it exists.
pub fn main_bu_path(dir: &Path) -> Option<PathBuf> {
    let p = dir.join("main.bu");
    if p.exists() { Some(p) } else { None }
}

/// Collect all .bu files, excluding inventory.bu AND main.bu.
pub fn collect_bu_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().map(|x| x == "bu").unwrap_or(false)
                && p.file_name().and_then(|n| n.to_str())
                    .map(|n| n != "inventory.bu" && n != "main.bu")
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

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None    => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// ── Direct single-file validation (for `bullang file`) ───────────────────────

pub fn validate_source_direct(
    sf:       &SourceFile,
    path:     &str,
    callable: &HashSet<String>,
    rank:     &Rank,
) -> Vec<ValidationError> {
    let mut errors  = Vec::new();
    let is_skirmish = rank == &Rank::Skirmish;

    if sf.bullets.len() > 5 {
        errors.push(ferr(path, format!(
            "A source file cannot contain more than 5 functions (found {}).",
            sf.bullets.len()
        )));
    }

    for func in &sf.bullets {
        errors.extend(validate_function(func, path, callable, is_skirmish));
    }

    errors
}
