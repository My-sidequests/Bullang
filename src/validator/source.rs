//! Source file, function, and bullet-level structural validation.

use std::path::Path;
use std::collections::HashSet;
use std::fs;
use crate::ast::*;
use crate::parser;
use super::{ValidationError, AllErrors};

// ── Source file ───────────────────────────────────────────────────────────────

pub fn validate_source_file(
    path:           &Path,
    folder_rank:    &Rank,
    _inv_map:       &std::collections::HashMap<String, Vec<String>>,
    child_callable: &HashSet<String>,
    folder_lang:    Option<&crate::ast::Backend>,
) -> AllErrors {
    let mut all = AllErrors::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => {
            all.push_structural(super::err(path, format!("Could not read file: {}", e)));
            return all;
        }
    };

    let path_str = path.display().to_string();
    let result   = parser::parse_file_tolerant(&source, &path_str);
    all.extend_parse(result.errors);

    let sf = match result.file {
        BuFile::Source(s) => s,
        _                 => return all,
    };

    let is_skirmish = folder_rank == &Rank::Skirmish;

    if sf.bullets.len() > 5 {
        all.push_structural(ferr(&path_str, format!(
            "A source file cannot contain more than 5 functions (found {}).",
            sf.bullets.len()
        )));
    }

    for func in &sf.bullets {
        all.extend_structural(validate_function(func, &path_str, child_callable, is_skirmish));
        // Native block language check
        if let Some(lang) = folder_lang {
            all.extend_structural(validate_native_blocks_lang(func, &path_str, lang));
        } else {
            // No lang declared — native blocks require one
            if let crate::ast::BulletBody::Natives(blocks) = &func.body {
                if !blocks.is_empty() {
                    all.push_structural(ferr(&path_str, format!(
                        "Function '{}': native block '@{}' requires #lang: to be \
                         declared in this folder's inventory.",
                        func.name, blocks[0].backend.escape_keyword()
                    )));
                }
            }
        }
    }

    all
}

// ── Native block language enforcement ────────────────────────────────────────

/// Every native block in the function must match the folder's declared language.
/// `@c` is accepted in a `#lang: cpp` folder.
fn validate_native_blocks_lang(
    func:    &Bullet,
    path:    &str,
    lang:    &crate::ast::Backend,
) -> Vec<ValidationError> {
    let blocks = match &func.body {
        crate::ast::BulletBody::Natives(b) => b,
        _                                   => return vec![],
    };

    let mut errors = Vec::new();
    for block in blocks {
        let ok = match (&block.backend, lang) {
            // C blocks are valid in C++ folders
            (crate::ast::Backend::C, crate::ast::Backend::Cpp) => true,
            (a, b) => a == b,
        };
        if !ok {
            errors.push(ferr(path, format!(
                "Function '{}': '@{}' block is not allowed in a '#lang: {}' folder. \
                 Use '@{}' instead, or move this function to a folder with '#lang: {}'.",
                func.name,
                block.backend.escape_keyword(),
                lang.ext(),
                lang.escape_keyword(),
                block.backend.ext(),
            )));
        }
    }
    errors
}

// ── Function ──────────────────────────────────────────────────────────────────

pub fn validate_function(
    func:        &Bullet,
    path:        &str,
    callable:    &HashSet<String>,
    is_skirmish: bool,
) -> Vec<ValidationError> {
    match &func.body {
        BulletBody::Natives(blocks) => {
            match blocks.iter().find(|b| matches!(b.backend, crate::ast::Backend::Unknown(_))) {
                Some(b) => {
                    if let crate::ast::Backend::Unknown(kw) = &b.backend {
                        vec![ferr(path, format!(
                            "Function '{}': '@{}' is not a supported backend. \
                             Supported escape blocks: @rust, @python, @c, @cpp, @go.",
                            func.name, kw
                        ))]
                    } else { vec![] }
                }
                None => vec![],
            }
        }
        BulletBody::Builtin(name) => {
            if !crate::stdlib::is_known_builtin(name) {
                vec![ferr(path, format!(
                    "Function '{}': 'builtin::{}' is not a known builtin. \
                     Run `bullang stdlib --list` to see available builtins.",
                    func.name, name
                ))]
            } else {
                vec![]
            }
        }
        BulletBody::Pipes(pipes) => validate_bullets(
            pipes, &func.name, &func.output.name,
            &func.params, path, callable, is_skirmish,
        ),
    }
}

// ── Bullets ───────────────────────────────────────────────────────────────────

pub fn validate_bullets(
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
        // A `?` binding is consumed by the propagation check itself — not by a later bullet
        if bullet.propagate {
            consumed.insert(bullet.binding.clone());
        }
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

// ── Call / atom traversal ─────────────────────────────────────────────────────

pub fn collect_call_errors(
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

pub fn check_atom(
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

// ── Local error constructors ──────────────────────────────────────────────────

fn serr(file: &str, span: Span, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: file.to_string(), line: span.line, col: span.col, message: msg.into() }
}

fn ferr(file: &str, msg: impl Into<String>) -> ValidationError {
    ValidationError { file: file.to_string(), line: 0, col: 0, message: msg.into() }
}
