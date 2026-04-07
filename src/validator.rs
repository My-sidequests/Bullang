//! Compile-time structural validation for the Bullang hierarchy.
//!
//! Works with any rank as root — war, theater, tactic, skirmish, etc.
//! Runs bottom-up: deepest folders first, errors bubble upward.

use std::path::{Path, PathBuf};
use std::collections::HashSet;
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

/// Validate a Bullang root of any rank.
pub fn validate_tree(root: &Path) -> Vec<ValidationError> {
    validate_folder(root)
}

// ── Folder validation (recursive, bottom-up) ──────────────────────────────────

fn validate_folder(dir: &Path) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let rank = match read_folder_rank(dir) {
        Some(r) => r,
        None => {
            errors.push(err(dir, "folder has no inventory.bu or inventory.bu is missing #rank"));
            return errors;
        }
    };

    let subdirs  = collect_subdirs(dir);
    let bu_files = collect_bu_files(dir);

    // Recurse into sub-folders first (bottom-up)
    for subdir in &subdirs {
        errors.extend(validate_folder(subdir));
    }

    // ── War: no .bu files, only theater sub-folders ───────────────────────────
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
        for subdir in &subdirs {
            validate_child_rank(subdir, &Rank::Theater, &mut errors);
        }
        return errors;
    }

    // ── Skirmish: no sub-folders, only .bu files ──────────────────────────────
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
        for bu in &bu_files {
            errors.extend(validate_bu_file(bu, &rank, &HashSet::new()));
        }
        return errors;
    }

    // ── Middle ranks: up to 5 sub-folders + up to 5 .bu files ────────────────
    let child_rank = rank.child_rank().unwrap();

    if subdirs.len() > 5 {
        errors.push(err(dir, format!(
            "{} folder may contain at most 5 {} sub-folders, found {}",
            rank.name(), child_rank.name(), subdirs.len()
        )));
    }
    if bu_files.len() > 5 {
        errors.push(err(dir, format!(
            "{} folder may contain at most 5 .bu files, found {}",
            rank.name(), bu_files.len()
        )));
    }
    for subdir in &subdirs {
        validate_child_rank(subdir, &child_rank, &mut errors);
    }

    // Inventory context for .bu files at this level = everything below
    let child_inv = collect_child_exports(&subdirs);
    for bu in &bu_files {
        errors.extend(validate_bu_file(bu, &rank, &child_inv));
    }

    errors
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
                "sub-folder is missing inventory.bu (expected a {} folder)",
                expected.name()
            )));
        }
    }
}

// ── .bu file validation ───────────────────────────────────────────────────────

fn validate_bu_file(
    path:        &Path,
    folder_rank: &Rank,
    inv_names:   &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => { errors.push(err(path, format!("could not read file: {}", e))); return errors; }
    };

    let sk = match parser::parse_file(&source, false) {
        Ok(BuFile::Skirmish(s))  => s,
        Ok(BuFile::Inventory(_)) => return errors,
        Err(e) => { errors.push(err(path, format!("parse error: {}", e))); return errors; }
    };

    let path_str = path.display().to_string();

    if &sk.rank != folder_rank {
        errors.push(ferr(&path_str, format!(
            "file declares #rank: {} but lives in a {} folder",
            sk.rank.name(), folder_rank.name()
        )));
    }

    if sk.bullets.len() > 5 {
        errors.push(ferr(&path_str, format!(
            "a .bu file may contain at most 5 bullets, found {}", sk.bullets.len()
        )));
    }

    for name in &sk.exports {
        if !sk.bullets.iter().any(|b| &b.name == name) {
            errors.push(ferr(&path_str, format!("#export '{}' has no matching bullet", name)));
        }
    }

    for bullet in &sk.bullets {
        errors.extend(validate_bullet(bullet, &path_str, inv_names, folder_rank));
    }

    errors
}

// ── Bullet validation ─────────────────────────────────────────────────────────

fn validate_bullet(
    bullet:      &Bullet,
    path:        &str,
    inv_names:   &HashSet<String>,
    folder_rank: &Rank,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let pipes = match &bullet.body {
        BulletBody::Native { .. }  => return errors,
        BulletBody::Pipes(p)       => p,
    };

    let empty   = HashSet::new();
    let allowed = if folder_rank == &Rank::Skirmish { &empty } else { inv_names };

    errors.extend(validate_pipes(
        pipes,
        &bullet.name,
        &bullet.output.name,
        &bullet.params,
        path,
        allowed,
        folder_rank == &Rank::Skirmish,
    ));

    errors
}

fn validate_pipes(
    pipes:       &[Pipe],
    bullet_name: &str,
    output_name: &str,
    params:      &[Param],
    path:        &str,
    inv_names:   &HashSet<String>,
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

        collect_call_errors(&pipe.expr, bullet_name, path, pipe.span, inv_names, is_skirmish, &mut errors);

        if bound.contains(&pipe.binding) {
            errors.push(serr(path, pipe.span, format!(
                "bullet '{}': '{{{}}}' is assigned more than once", bullet_name, pipe.binding
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
                "bullet '{}': '{{{}}}' is produced but never consumed", bullet_name, b
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
    inv_names:   &HashSet<String>,
    is_skirmish: bool,
    errors:      &mut Vec<ValidationError>,
) {
    match expr {
        Expr::Atom(a)      => check_atom(a, bullet_name, path, span, inv_names, is_skirmish, errors),
        Expr::BinOp(b)     => {
            check_atom(&b.lhs, bullet_name, path, span, inv_names, is_skirmish, errors);
            check_atom(&b.rhs, bullet_name, path, span, inv_names, is_skirmish, errors);
        }
        Expr::Tuple(exprs) => {
            for e in exprs {
                collect_call_errors(e, bullet_name, path, span, inv_names, is_skirmish, errors);
            }
        }
    }
}

fn check_atom(
    atom:        &Atom,
    bullet_name: &str,
    path:        &str,
    span:        Span,
    inv_names:   &HashSet<String>,
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
        if !inv_names.is_empty() && !inv_names.contains(name.as_str()) {
            errors.push(serr(path, span, format!(
                "bullet '{}': calls '{}' which is not exported by any child inventory",
                bullet_name, name
            )));
        }
        for arg in args {
            if let CallArg::BulletRef(r) = arg {
                if !inv_names.is_empty() && !inv_names.contains(r.as_str()) {
                    errors.push(serr(path, span, format!(
                        "bullet '{}': references '&{}' which is not exported by any child inventory",
                        bullet_name, r
                    )));
                }
            }
        }
    }
}

// ── Export collection ─────────────────────────────────────────────────────────

fn collect_child_exports(subdirs: &[PathBuf]) -> HashSet<String> {
    let mut names = HashSet::new();
    for subdir in subdirs {
        if subdir.join("inventory.bu").exists() {
            names.extend(collect_folder_exports(subdir));
        }
    }
    names
}

pub fn collect_folder_exports(dir: &Path) -> HashSet<String> {
    let mut names = HashSet::new();
    for bu in collect_bu_files(dir) {
        if let Ok(source) = fs::read_to_string(&bu) {
            if let Ok(BuFile::Skirmish(sk)) = parser::parse_file(&source, false) {
                for export in sk.exports { names.insert(export); }
            }
        }
    }
    for subdir in collect_subdirs(dir) {
        names.extend(collect_folder_exports(&subdir));
    }
    names
}

// ── Filesystem helpers ────────────────────────────────────────────────────────

pub fn read_folder_rank(dir: &Path) -> Option<Rank> {
    let source = fs::read_to_string(dir.join("inventory.bu")).ok()?;
    match parser::parse_file(&source, true) {
        Ok(BuFile::Inventory(inv)) => Some(inv.rank),
        _                          => None,
    }
}

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

pub fn validate_bu_file_direct(
    sk:          &SkirmishFile,
    path:        &str,
    inv_names:   &HashSet<String>,
    folder_rank: &Rank,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if sk.bullets.len() > 5 {
        errors.push(ferr(path, format!(
            "a .bu file may contain at most 5 bullets, found {}", sk.bullets.len()
        )));
    }
    for name in &sk.exports {
        if !sk.bullets.iter().any(|b| &b.name == name) {
            errors.push(ferr(path, format!("#export '{}' has no matching bullet", name)));
        }
    }
    for bullet in &sk.bullets {
        errors.extend(validate_bullet(bullet, path, inv_names, folder_rank));
    }

    errors
}
