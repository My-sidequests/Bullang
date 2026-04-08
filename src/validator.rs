//! Compile-time structural validation.
//!
//! Key rules:
//!   - inventory.bu is the mandatory complete manifest of its folder.
//!   - Every .bu file in a folder must appear in inventory.
//!   - Every function in inventory must exist in the corresponding file.
//!   - Every function in a file must be listed in inventory (no hidden code).
//!   - File rank is inferred from folder rank — never declared in source files.
//!   - Hierarchy limits: 5 files max per folder, 5 sub-folders max, etc.

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

    // Read and validate inventory first
    let inv = match read_inventory(dir) {
        Ok(i)  => i,
        Err(e) => { errors.push(err(dir, e)); return errors; }
    };

    let subdirs  = collect_subdirs(dir);
    let bu_files = collect_bu_files(dir);

    // Recurse into sub-folders first (bottom-up)
    for subdir in &subdirs {
        errors.extend(validate_folder(subdir));
    }

    match inv.rank {
        // ── War: only sub-folders ─────────────────────────────────────────────
        Rank::War => {
            if !bu_files.is_empty() {
                errors.push(err(dir, format!(
                    "war folder may not contain source files (found {}); \
                     only theater sub-folders are allowed",
                    bu_files.len()
                )));
            }
            if subdirs.len() > 5 {
                errors.push(err(dir, format!(
                    "war folder may contain at most 5 theater folders, found {}",
                    subdirs.len()
                )));
            }
            // Inventory entries must be empty for war
            if !inv.entries.is_empty() {
                errors.push(err(
                    &dir.join("inventory.bu"),
                    "war inventory must not list any files — war folders contain only sub-folders"
                ));
            }
            for subdir in &subdirs {
                validate_child_rank(subdir, &Rank::Theater, &mut errors);
            }
        }

        // ── Skirmish: only source files ───────────────────────────────────────
        Rank::Skirmish => {
            if !subdirs.is_empty() {
                errors.push(err(dir, format!(
                    "skirmish folder may not contain sub-folders (found {}); \
                     only source files are allowed",
                    subdirs.len()
                )));
            }
            if bu_files.len() > 5 {
                errors.push(err(dir, format!(
                    "skirmish folder may contain at most 5 source files, found {}",
                    bu_files.len()
                )));
            }
            // Validate inventory completeness
            errors.extend(validate_inventory_completeness(
                dir, &inv, &bu_files, &[]
            ));
            // Validate each source file
            let inv_map = build_inv_map(&inv);
            for bu in &bu_files {
                errors.extend(validate_source_file(bu, &inv.rank, &inv_map, &HashSet::new()));
            }
        }

        // ── Middle ranks ─────────────────────────────────────────────────────
        ref rank => {
            let child_rank = rank.child_rank().unwrap();

            if subdirs.len() > 5 {
                errors.push(err(dir, format!(
                    "{} folder may contain at most 5 {} sub-folders, found {}",
                    rank.name(), child_rank.name(), subdirs.len()
                )));
            }
            if bu_files.len() > 5 {
                errors.push(err(dir, format!(
                    "{} folder may contain at most 5 source files, found {}",
                    rank.name(), bu_files.len()
                )));
            }
            for subdir in &subdirs {
                validate_child_rank(subdir, &child_rank, &mut errors);
            }

            // Validate inventory completeness
            errors.extend(validate_inventory_completeness(
                dir, &inv, &bu_files, &subdirs
            ));

            // Callable names from child folder inventories
            let child_callable = collect_child_callable(&subdirs);

            // Validate each source file
            let inv_map = build_inv_map(&inv);
            for bu in &bu_files {
                errors.extend(validate_source_file(bu, rank, &inv_map, &child_callable));
            }
        }
    }

    errors
}

// ── Inventory completeness validation ────────────────────────────────────────

/// Build a map of filename → expected functions from inventory entries.
fn build_inv_map(inv: &InventoryFile) -> HashMap<String, Vec<String>> {
    inv.entries.iter()
        .map(|e| (e.file.clone(), e.functions.clone()))
        .collect()
}

/// Validate that inventory and filesystem are in perfect sync:
///   - Every .bu file appears in inventory
///   - Every inventory entry has a matching .bu file
///   - For each file: inventory lists exactly the functions that exist
fn validate_inventory_completeness(
    dir:      &Path,
    inv:      &InventoryFile,
    bu_files: &[PathBuf],
    _subdirs: &[PathBuf],
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let inv_path   = dir.join("inventory.bu");
    let inv_str    = inv_path.display().to_string();

    // Build sets of stems
    let file_stems: HashSet<String> = bu_files.iter()
        .filter_map(|p| p.file_stem()?.to_str().map(|s| s.to_string()))
        .collect();

    let inv_stems: HashSet<String> = inv.entries.iter()
        .map(|e| e.file.clone())
        .collect();

    // Files present on disk but not in inventory
    for stem in &file_stems {
        if !inv_stems.contains(stem) {
            errors.push(ferr(&inv_str, format!(
                "source file '{}.bu' exists but is not listed in inventory — \
                 add a line: {}: <fn1, fn2, ...>;",
                stem, stem
            )));
        }
    }

    // Inventory entries with no matching file on disk
    for stem in &inv_stems {
        if !file_stems.contains(stem) {
            errors.push(ferr(&inv_str, format!(
                "inventory lists '{}' but '{}.bu' does not exist in this folder",
                stem, stem
            )));
        }
    }

    // For each file that appears in both: validate function lists match
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

        // Functions in file but not in inventory
        for name in &actual_fns {
            if !listed_fns.contains(name) {
                errors.push(ferr(&inv_str, format!(
                    "function '{}' exists in '{}.bu' but is not listed in inventory — \
                     all functions must be listed",
                    name, entry.file
                )));
            }
        }

        // Functions in inventory but not in file
        for name in &listed_fns {
            if !actual_fns.contains(name) {
                errors.push(ferr(&inv_str, format!(
                    "inventory lists '{}' under '{}' but that function does not exist in '{}.bu'",
                    name, entry.file, entry.file
                )));
            }
        }
    }

    errors
}

// ── Source file validation ────────────────────────────────────────────────────

fn validate_source_file(
    path:         &Path,
    folder_rank:  &Rank,
    inv_map:      &HashMap<String, Vec<String>>,
    child_callable: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => { errors.push(err(path, format!("could not read: {}", e))); return errors; }
    };

    let sf = match parser::parse_file(&source, false) {
        Ok(BuFile::Source(s)) => s,
        Ok(BuFile::Inventory(_)) => return errors,
        Err(e) => { errors.push(err(path, format!("parse error: {}", e))); return errors; }
    };

    let path_str = path.display().to_string();

    if sf.bullets.len() > 5 {
        errors.push(ferr(&path_str, format!(
            "a source file may contain at most 5 bullets, found {}",
            sf.bullets.len()
        )));
    }

    for bullet in &sf.bullets {
        errors.extend(validate_bullet(
            bullet, &path_str, child_callable,
            folder_rank == &Rank::Skirmish,
        ));
    }

    errors
}

// ── Bullet validation ─────────────────────────────────────────────────────────

fn validate_bullet(
    bullet:      &Bullet,
    path:        &str,
    callable:    &HashSet<String>,
    is_skirmish: bool,
) -> Vec<ValidationError> {
    match &bullet.body {
        BulletBody::Native { .. } => vec![],
        BulletBody::Pipes(pipes) => validate_pipes(
            pipes, &bullet.name, &bullet.output.name,
            &bullet.params, path, callable, is_skirmish,
        ),
    }
}

fn validate_pipes(
    pipes:       &[Pipe],
    bullet_name: &str,
    output_name: &str,
    params:      &[Param],
    path:        &str,
    callable:    &HashSet<String>,
    is_skirmish: bool,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if pipes.len() > 5 {
        errors.push(ferr(path, format!(
            "bullet '{}': may contain at most 5 pipe statements, found {}",
            bullet_name, pipes.len()
        )));
    }

    let param_names: HashSet<&str> = params.iter().map(|p| p.name.as_str()).collect();
    let mut bound:    HashSet<String> = HashSet::new();
    let mut consumed: HashSet<String> = HashSet::new();
    let last = pipes.len().saturating_sub(1);

    for (i, pipe) in pipes.iter().enumerate() {
        for input in &pipe.inputs {
            if param_names.contains(input.as_str()) {
                consumed.insert(input.clone());
            } else if bound.contains(input.as_str()) {
                consumed.insert(input.clone());
            } else {
                errors.push(serr(path, pipe.span, format!(
                    "bullet '{}' pipe {}: '{}' is not a declared param or earlier binding",
                    bullet_name, i + 1, input
                )));
            }
        }

        collect_call_errors(
            &pipe.expr, bullet_name, path, pipe.span,
            callable, is_skirmish, &mut errors,
        );

        if bound.contains(&pipe.binding) {
            errors.push(serr(path, pipe.span, format!(
                "bullet '{}': '{{{}}}' is assigned more than once",
                bullet_name, pipe.binding
            )));
        }

        if i == last && pipe.binding != output_name {
            errors.push(serr(path, pipe.span, format!(
                "bullet '{}': last pipe binds '{{{}}}' but declared output is '{{{}}}'",
                bullet_name, pipe.binding, output_name
            )));
        }

        bound.insert(pipe.binding.clone());
    }

    for b in &bound {
        if b != output_name && !consumed.contains(b) {
            errors.push(ferr(path, format!(
                "bullet '{}': '{{{}}}' is produced but never consumed",
                bullet_name, b
            )));
        }
    }

    errors
}

fn collect_call_errors(
    expr:        &Expr,
    bullet_name: &str,
    path:        &str,
    span:        Span,
    callable:    &HashSet<String>,
    is_skirmish: bool,
    errors:      &mut Vec<ValidationError>,
) {
    match expr {
        Expr::Atom(a)      => check_atom(a, bullet_name, path, span, callable, is_skirmish, errors),
        Expr::BinOp(b)     => {
            check_atom(&b.lhs, bullet_name, path, span, callable, is_skirmish, errors);
            check_atom(&b.rhs, bullet_name, path, span, callable, is_skirmish, errors);
        }
        Expr::Tuple(exprs) => {
            for e in exprs {
                collect_call_errors(e, bullet_name, path, span, callable, is_skirmish, errors);
            }
        }
    }
}

fn check_atom(
    atom:        &Atom,
    bullet_name: &str,
    path:        &str,
    span:        Span,
    callable:    &HashSet<String>,
    is_skirmish: bool,
    errors:      &mut Vec<ValidationError>,
) {
    if let Atom::Call { name, args } = atom {
        if is_skirmish {
            errors.push(serr(path, span, format!(
                "bullet '{}': skirmish files may not call other bullets \
                 (found call to '{}'). Use raw expressions only.",
                bullet_name, name
            )));
            return;
        }
        if !callable.is_empty() && !callable.contains(name.as_str()) {
            errors.push(serr(path, span, format!(
                "bullet '{}': calls '{}' which is not listed in any child inventory",
                bullet_name, name
            )));
        }
        for arg in args {
            if let CallArg::BulletRef(r) = arg {
                if !callable.is_empty() && !callable.contains(r.as_str()) {
                    errors.push(serr(path, span, format!(
                        "bullet '{}': references '&{}' which is not listed in any child inventory",
                        bullet_name, r
                    )));
                }
            }
        }
    }
}

// ── Child callable collection ─────────────────────────────────────────────────

/// Collect all function names listed in child folder inventories.
/// These are the names a file at the current level is allowed to call.
pub fn collect_child_callable(subdirs: &[PathBuf]) -> HashSet<String> {
    let mut names = HashSet::new();
    for subdir in subdirs {
        if let Ok(inv) = read_inventory(subdir) {
            for entry in &inv.entries {
                for func in &entry.functions {
                    names.insert(func.clone());
                }
            }
            // Also recurse deeper — functions at any depth are callable
            names.extend(collect_child_callable(&collect_subdirs(subdir)));
        }
    }
    names
}

/// Collect all function names from a folder and all its descendants.
/// Used by the type checker to build the full TypeEnv.
pub fn collect_folder_callable(dir: &Path) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Ok(inv) = read_inventory(dir) {
        for entry in &inv.entries {
            for func in &entry.functions {
                names.insert(func.clone());
            }
        }
    }
    for subdir in collect_subdirs(dir) {
        names.extend(collect_folder_callable(&subdir));
    }
    names
}

// ── Inventory / rank helpers ──────────────────────────────────────────────────

pub fn read_inventory(dir: &Path) -> Result<InventoryFile, String> {
    let inv_path = dir.join("inventory.bu");
    let source   = fs::read_to_string(&inv_path)
        .map_err(|_| format!(
            "missing inventory.bu in '{}' — every Bullang folder must have one",
            dir.display()
        ))?;
    match parser::parse_file(&source, true) {
        Ok(BuFile::Inventory(inv)) => Ok(inv),
        Ok(_)  => Err(format!("inventory.bu in '{}' parsed as a source file", dir.display())),
        Err(e) => Err(format!("parse error in inventory.bu: {}", e)),
    }
}

pub fn read_folder_rank(dir: &Path) -> Option<Rank> {
    read_inventory(dir).ok().map(|inv| inv.rank)
}

fn validate_child_rank(subdir: &Path, expected: &Rank, errors: &mut Vec<ValidationError>) {
    match read_folder_rank(subdir) {
        Some(ref actual) if actual == expected => {}
        Some(ref actual) => {
            errors.push(err(subdir, format!(
                "expected a {} folder here, but inventory.bu declares #rank: {}",
                expected.name(), actual.name()
            )));
        }
        None => {
            errors.push(err(subdir, format!(
                "sub-folder '{}' is missing inventory.bu (expected a {} folder)",
                subdir.display(), expected.name()
            )));
        }
    }
}

// ── Filesystem helpers ────────────────────────────────────────────────────────

pub fn collect_bu_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().map(|x| x == "bu").unwrap_or(false)
                && p.file_name().and_then(|n| n.to_str())
                    .map(|n| n != "inventory.bu").unwrap_or(false)
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

// ── Direct single-file validation (for `bullang file`) ───────────────────────

pub fn validate_source_direct(
    sf:       &SourceFile,
    path:     &str,
    callable: &HashSet<String>,
    rank:     &Rank,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let inv_map: HashMap<String, Vec<String>> = HashMap::new();
    let is_skirmish = rank == &Rank::Skirmish;

    if sf.bullets.len() > 5 {
        errors.push(ferr(path, format!(
            "a source file may contain at most 5 bullets, found {}", sf.bullets.len()
        )));
    }

    for bullet in &sf.bullets {
        errors.extend(validate_bullet(bullet, path, callable, is_skirmish));
    }

    let _ = inv_map;
    errors
}
