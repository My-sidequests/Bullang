//! Compile-time validation for the Bullang hierarchy.
//!
//! Validation runs bottom-up: skirmish folders first, then tactic, up to war.
//! Each level checks its own rules before its parent checks it.

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::fs;
use crate::ast::*;
use crate::parser;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ValidationError {
    pub file:    String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.file, self.message)
    }
}

fn err(path: &Path, message: impl Into<String>) -> ValidationError {
    ValidationError {
        file:    path.display().to_string(),
        message: message.into(),
    }
}

fn ferr(file: &str, message: impl Into<String>) -> ValidationError {
    ValidationError {
        file:    file.to_string(),
        message: message.into(),
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Validate the entire war tree bottom-up.
/// Returns all errors found across the whole tree.
pub fn validate_tree(war_root: &Path) -> Vec<ValidationError> {
    validate_folder(war_root)
}

// ── Folder validation (recursive, bottom-up) ──────────────────────────────────

/// Validate a single folder and everything inside it.
/// Recurses into children first — errors bubble up from the bottom.
fn validate_folder(dir: &Path) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let rank = match read_folder_rank(dir) {
        Some(r) => r,
        None => {
            errors.push(err(dir, "folder has no inventory.bu or inventory.bu is missing #rank"));
            return errors;
        }
    };

    // ── Collect children ──────────────────────────────────────────────────────
    let subdirs  = collect_subdirs(dir);
    let bu_files = collect_bu_files(dir);

    // ── Recurse into subfolders FIRST (bottom-up) ─────────────────────────────
    for subdir in &subdirs {
        errors.extend(validate_folder(subdir));
    }

    // ── War: no .bu files allowed (only inventory.bu) ─────────────────────────
    if rank == Rank::War {
        if !bu_files.is_empty() {
            errors.push(err(dir, format!(
                "war folder may not contain .bu files (found {}); \
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
        // Validate that each subdir is a theater
        for subdir in &subdirs {
            validate_child_folder_rank(subdir, &Rank::Theater, &mut errors);
        }
        return errors;
    }

    // ── Skirmish: no sub-folders allowed ──────────────────────────────────────
    if rank == Rank::Skirmish {
        if !subdirs.is_empty() {
            errors.push(err(dir, format!(
                "skirmish folder may not contain sub-folders (found {}); \
                 only .bu files are allowed",
                subdirs.len()
            )));
        }
        if bu_files.len() > 5 {
            errors.push(err(dir, format!(
                "skirmish folder may contain at most 5 .bu files, found {}",
                bu_files.len()
            )));
        }
        // Validate each .bu file in this skirmish folder
        for bu_file in &bu_files {
            errors.extend(validate_bu_file(bu_file, &rank, &HashSet::new()));
        }
        return errors;
    }

    // ── All other ranks: up to 5 sub-folders + up to 5 .bu files ─────────────
    let expected_child_folder_rank = rank.child_rank().unwrap();

    if subdirs.len() > 5 {
        errors.push(err(dir, format!(
            "{} folder may contain at most 5 {} sub-folders, found {}",
            rank.name(), expected_child_folder_rank.name(), subdirs.len()
        )));
    }

    if bu_files.len() > 5 {
        errors.push(err(dir, format!(
            "{} folder may contain at most 5 .bu files, found {}",
            rank.name(), bu_files.len()
        )));
    }

    // Validate that each subdir is the expected child rank
    for subdir in &subdirs {
        validate_child_folder_rank(subdir, &expected_child_folder_rank, &mut errors);
    }

    // Build the inventory context for .bu files in this folder:
    // everything exported by child folder inventories
    let child_inventory = collect_child_inventory_exports(dir, &subdirs);

    // Validate each .bu file
    for bu_file in &bu_files {
        errors.extend(validate_bu_file(bu_file, &rank, &child_inventory));
    }

    errors
}

// ── Child folder rank check ───────────────────────────────────────────────────

fn validate_child_folder_rank(
    subdir:        &Path,
    expected_rank: &Rank,
    errors:        &mut Vec<ValidationError>,
) {
    match read_folder_rank(subdir) {
        Some(ref actual) if actual == expected_rank => {}
        Some(ref actual) => {
            errors.push(err(subdir, format!(
                "expected a {} folder here, but inventory.bu declares #rank: {}",
                expected_rank.name(), actual.name()
            )));
        }
        None => {
            errors.push(err(subdir, format!(
                "sub-folder is missing inventory.bu (expected a {} folder)",
                expected_rank.name()
            )));
        }
    }
}

// ── .bu file validation ───────────────────────────────────────────────────────

/// Validate a single .bu file.
/// `folder_rank`      — the rank of the containing folder
/// `inventory_names`  — names available from child inventories
fn validate_bu_file(
    path:            &Path,
    folder_rank:     &Rank,
    inventory_names: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => {
            errors.push(err(path, format!("could not read file: {}", e)));
            return errors;
        }
    };

    let bu = match parser::parse_file(&source, false) {
        Ok(f)  => f,
        Err(e) => {
            errors.push(err(path, format!("parse error: {}", e)));
            return errors;
        }
    };

    let sk = match bu {
        BuFile::Skirmish(s) => s,
        BuFile::Inventory(_) => return errors, // inventories validated separately
    };

    // File rank must match folder rank
    if &sk.rank != folder_rank {
        errors.push(err(path, format!(
            "file declares #rank: {} but lives in a {} folder",
            sk.rank.name(), folder_rank.name()
        )));
    }

    // Max 5 bullets per file
    if sk.bullets.len() > 5 {
        errors.push(err(path, format!(
            "a .bu file may contain at most 5 bullets, found {}",
            sk.bullets.len()
        )));
    }

    // Every exported name must have a matching bullet
    for name in &sk.exports {
        if !sk.bullets.iter().any(|b| &b.name == name) {
            errors.push(err(path, format!(
                "#export '{}' has no matching bullet", name
            )));
        }
    }

    // Validate each bullet
    for bullet in &sk.bullets {
        errors.extend(validate_bullet(bullet, path, inventory_names, folder_rank));
    }

    errors
}

// ── Bullet validation ─────────────────────────────────────────────────────────

fn validate_bullet(
    bullet:          &Bullet,
    path:            &Path,
    inventory_names: &HashSet<String>,
    folder_rank:     &Rank,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    match &bullet.body {
        // Native blocks: the developer owns the implementation.
        // We still validate the signature is complete.
        BulletBody::Native { .. } => {}

        BulletBody::Pipes(pipes) => {
            // Skirmish files must not call other bullets (no imports)
            let empty = HashSet::new();
            let allowed_calls = if folder_rank == &Rank::Skirmish {
                &empty
            } else {
                inventory_names
            };

            errors.extend(validate_pipes(
                pipes,
                &bullet.name,
                &bullet.output.name,
                &bullet.params,
                &path.display().to_string(),
                allowed_calls,
                folder_rank == &Rank::Skirmish,
            ));
        }
    }

    errors
}

fn validate_pipes(
    pipes:            &[Pipe],
    bullet_name:      &str,
    output_name:      &str,
    params:           &[Param],
    path:             &str,
    inventory_names:  &HashSet<String>,
    is_skirmish:      bool,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Max 5 pipes per bullet
    if pipes.len() > 5 {
        errors.push(ferr(path, format!(
            "bullet \'{}\': may contain at most 5 pipe statements, found {}",
            bullet_name, pipes.len()
        )));
    }

    let param_names: HashSet<&str> = params.iter()
        .map(|p| p.name.as_str())
        .collect();

    let mut bound:    HashSet<String> = HashSet::new();
    let mut consumed: HashSet<String> = HashSet::new();
    let last = pipes.len().saturating_sub(1);

    for (i, pipe) in pipes.iter().enumerate() {
        // Each input must be a declared param or an earlier binding
        for input in &pipe.inputs {
            if param_names.contains(input.as_str()) {
                consumed.insert(input.clone());
            } else if bound.contains(input.as_str()) {
                consumed.insert(input.clone());
            } else {
                errors.push(ferr(path, format!(
                    "bullet '{}' pipe {}: '{}' is not a declared param or earlier binding",
                    bullet_name, i + 1, input
                )));
            }
        }

        // Validate calls inside the expression
        collect_call_errors(
            &pipe.expr,
            bullet_name,
            path,
            inventory_names,
            is_skirmish,
            &mut errors,
        );

        // No rebinding
        if bound.contains(&pipe.binding) {
            errors.push(ferr(path, format!(
                "bullet '{}': '{{{}}}' is assigned more than once",
                bullet_name, pipe.binding
            )));
        }

        // Last pipe must assign to the declared output name
        if i == last && pipe.binding != output_name {
            errors.push(ferr(path, format!(
                "bullet '{}': last pipe binds '{{{}}}' but declared output is '{{{}}}'",
                bullet_name, pipe.binding, output_name
            )));
        }

        bound.insert(pipe.binding.clone());
    }

    // Dead intermediate: every non-output binding must be consumed
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
    expr:            &Expr,
    bullet_name:     &str,
    path:            &str,
    inventory_names: &HashSet<String>,
    is_skirmish:     bool,
    errors:          &mut Vec<ValidationError>,
) {
    match expr {
        Expr::Atom(a)      => {
            check_atom(a, bullet_name, path, inventory_names, is_skirmish, errors);
        }
        Expr::BinOp(b)     => {
            check_atom(&b.lhs, bullet_name, path, inventory_names, is_skirmish, errors);
            check_atom(&b.rhs, bullet_name, path, inventory_names, is_skirmish, errors);
        }
        Expr::Tuple(exprs) => {
            for e in exprs {
                collect_call_errors(e, bullet_name, path, inventory_names, is_skirmish, errors);
            }
        }
    }
}

fn check_atom(
    atom:            &Atom,
    bullet_name:     &str,
    path:            &str,
    inventory_names: &HashSet<String>,
    is_skirmish:     bool,
    errors:          &mut Vec<ValidationError>,
) {
    if let Atom::Call { name, args } = atom {
        // Skirmish files may not call other bullets — only raw expressions
        if is_skirmish {
            errors.push(ferr(path, format!(
                "bullet '{}': skirmish files may not call other bullets \
                 (found call to '{}'). Use raw expressions only.",
                bullet_name, name
            )));
            return;
        }

        // Non-skirmish: called name must be in the child inventory
        if !inventory_names.is_empty() && !inventory_names.contains(name.as_str()) {
            errors.push(ferr(path, format!(
                "bullet '{}': calls '{}' which is not exported by any child inventory",
                bullet_name, name
            )));
        }

        // &bullet_ref targets must also be in the inventory
        for arg in args {
            if let CallArg::BulletRef(r) = arg {
                if !inventory_names.is_empty() && !inventory_names.contains(r.as_str()) {
                    errors.push(ferr(path, format!(
                        "bullet '{}': references '&{}' which is not exported \
                         by any child inventory",
                        bullet_name, r
                    )));
                }
            }
        }
    }
}

// ── Inventory export collection ───────────────────────────────────────────────

/// Collect all exports surfaced by child folder inventories.
/// This is what .bu files at this level are allowed to call.
fn collect_child_inventory_exports(
    _parent_dir: &Path,
    subdirs:     &[PathBuf],
) -> HashSet<String> {
    let mut names = HashSet::new();
    for subdir in subdirs {
        let inv_path = subdir.join("inventory.bu");
        if fs::read_to_string(&inv_path).is_ok() {
            // The inventory's exports are the union of everything its children export.
            // We reconstruct that by scanning the inventory folder's own .bu files
            // and sub-folder inventories recursively.
            names.extend(collect_folder_exports(subdir));
        }
    }
    names
}

/// Collect ALL exports that a folder surfaces upward through the hierarchy.
/// This includes:
/// - exports declared in .bu files directly in this folder
/// - exports from all descendant folders (recursively)
/// This ensures that a theater-level file can see everything exported
/// all the way down through battles, strategies, tactics, and skirmishes.
pub fn collect_folder_exports(dir: &Path) -> HashSet<String> {
    let mut names = HashSet::new();

    // Own .bu files at this level
    for bu_file in collect_bu_files(dir) {
        if let Ok(source) = fs::read_to_string(&bu_file) {
            if let Ok(BuFile::Skirmish(sk)) = parser::parse_file(&source, false) {
                for export in sk.exports {
                    names.insert(export);
                }
            }
        }
    }

    // All descendant folders — fully recursive so every level is visible
    for subdir in collect_subdirs(dir) {
        names.extend(collect_folder_exports(&subdir));
    }

    names
}

// ── Filesystem helpers ────────────────────────────────────────────────────────

/// Read the rank declared in a folder's inventory.bu.
pub fn read_folder_rank(dir: &Path) -> Option<Rank> {
    let inv_path = dir.join("inventory.bu");
    let source   = fs::read_to_string(&inv_path).ok()?;
    match parser::parse_file(&source, true) {
        Ok(BuFile::Inventory(inv)) => Some(inv.rank),
        _                          => None,
    }
}

/// Collect all .bu files in a directory, excluding inventory.bu. Sorted.
pub fn collect_bu_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().map(|x| x == "bu").unwrap_or(false)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n != "inventory.bu")
                    .unwrap_or(false)
        })
        .collect();
    files.sort();
    files
}

/// Collect immediate sub-directories. Sorted.
pub fn collect_subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

// ── Direct single-file validation (for `bullang file` command) ───────────────

/// Validate a pre-parsed SkirmishFile directly.
/// Used by the `file` subcommand where no folder context is available.
pub fn validate_bu_file_direct(
    sk:              &crate::ast::SkirmishFile,
    path:            &str,
    inventory_names: &HashSet<String>,
    folder_rank:     &Rank,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if sk.bullets.len() > 5 {
        errors.push(ferr(path, format!(
            "a .bu file may contain at most 5 bullets, found {}",
            sk.bullets.len()
        )));
    }

    for name in &sk.exports {
        if !sk.bullets.iter().any(|b| &b.name == name) {
            errors.push(ferr(path, format!(
                "#export '{}' has no matching bullet", name
            )));
        }
    }

    for bullet in &sk.bullets {
        errors.extend(validate_bullet(
            bullet,
            &PathBuf::from(path),
            inventory_names,
            folder_rank,
        ));
    }

    errors
}
