//! Type checking pass — runs after structural validation.
//!
//! Works with any rank as root.
//! All errors include file + line:col from the pipe's Span.

use std::collections::HashMap;
use std::path::Path;
use std::fs;

use crate::ast::*;
use crate::parser;
use crate::validator::{collect_bu_files, collect_subdirs, read_folder_rank};

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TypeError {
    pub file:    String,
    pub line:    usize,
    pub col:     usize,
    pub message: String,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(f, "[{}:{}:{}] {}", self.file, self.line, self.col, self.message)
        } else {
            write!(f, "[{}] {}", self.file, self.message)
        }
    }
}

fn terr(path: &str, span: Span, msg: impl Into<String>) -> TypeError {
    TypeError { file: path.to_string(), line: span.line, col: span.col, message: msg.into() }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Type-check a full tree rooted at any rank.
pub fn typecheck_tree(root: &Path) -> Vec<TypeError> {
    let mut errors = Vec::new();
    check_folder(root, &mut errors);
    errors
}

/// Type-check a single pre-parsed file with no cross-file context.
pub fn typecheck_file(sk: &SkirmishFile, path: &str) -> Vec<TypeError> {
    let mut errors = Vec::new();
    for bullet in &sk.bullets {
        errors.extend(check_bullet(bullet, path, &TypeEnv::new(), sk.rank == Rank::Skirmish));
    }
    errors
}

// ── Folder-level type checking ────────────────────────────────────────────────

fn check_folder(dir: &Path, errors: &mut Vec<TypeError>) -> TypeEnv {
    let rank = match read_folder_rank(dir) {
        Some(r) => r,
        None    => return TypeEnv::new(),
    };

    let mut env = TypeEnv::new();

    // War: only sub-folders, no .bu files
    if rank == Rank::War {
        for subdir in collect_subdirs(dir) {
            env.extend(check_folder(&subdir, errors));
        }
        return env;
    }

    // All other ranks: recurse into sub-folders first (bottom-up)
    if rank.has_sub_folders() {
        for subdir in collect_subdirs(dir) {
            env.extend(check_folder(&subdir, errors));
        }
    }

    // Type-check .bu files using the accumulated child env
    let is_skirmish = rank == Rank::Skirmish;
    for bu_path in collect_bu_files(dir) {
        let file_env = check_bu_file(&bu_path, &env, is_skirmish, errors);
        env.extend(file_env);
    }

    env
}

// ── File-level type checking ──────────────────────────────────────────────────

fn check_bu_file(
    path:        &Path,
    env:         &TypeEnv,
    is_skirmish: bool,
    errors:      &mut Vec<TypeError>,
) -> TypeEnv {
    let mut exported_env = TypeEnv::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(_) => return exported_env,
    };

    let sk = match parser::parse_file(&source, false) {
        Ok(BuFile::Skirmish(s))  => s,
        Ok(BuFile::Inventory(_)) => return exported_env,
        Err(_)                   => return exported_env,
    };

    let path_str = path.display().to_string();

    for bullet in &sk.bullets {
        errors.extend(check_bullet(bullet, &path_str, env, is_skirmish));
        if bullet.exported {
            exported_env.insert(bullet.name.clone(), BulletSig {
                params:  bullet.params.iter().map(|p| p.ty.clone()).collect(),
                returns: bullet.output.ty.clone(),
            });
        }
    }

    exported_env
}

// ── Bullet-level type checking ────────────────────────────────────────────────

fn check_bullet(
    bullet:      &Bullet,
    path:        &str,
    env:         &TypeEnv,
    is_skirmish: bool,
) -> Vec<TypeError> {
    let mut errors = Vec::new();

    let pipes = match &bullet.body {
        BulletBody::Native { .. } => return errors,
        BulletBody::Pipes(p)      => p,
    };

    let mut local: HashMap<String, BuType> = bullet.params.iter()
        .map(|p| (p.name.clone(), p.ty.clone()))
        .collect();

    let last = pipes.len().saturating_sub(1);

    for (i, pipe) in pipes.iter().enumerate() {
        let expr_type = infer_expr(
            &pipe.expr, &local, env, is_skirmish,
            &bullet.name, path, pipe.span, &mut errors,
        );

        // Last pipe: inferred type must match declared output type
        // (input types and output type are always independent — i16 in / Vec<u32> out is valid)
        if i == last && !types_compatible(&expr_type, &bullet.output.ty) {
            errors.push(terr(path, pipe.span, format!(
                "bullet '{}': last pipe produces {} but declared output is {}",
                bullet.name, expr_type.to_rust(), bullet.output.ty.to_rust()
            )));
        }

        local.insert(pipe.binding.clone(), expr_type);
    }

    errors
}

// ── Type inference ────────────────────────────────────────────────────────────

fn infer_expr(
    expr:        &Expr,
    local:       &HashMap<String, BuType>,
    env:         &TypeEnv,
    is_skirmish: bool,
    bullet_name: &str,
    path:        &str,
    span:        Span,
    errors:      &mut Vec<TypeError>,
) -> BuType {
    match expr {
        Expr::Atom(a) => infer_atom(a, local, env, is_skirmish, bullet_name, path, span, errors),

        Expr::BinOp(b) => {
            let lhs_ty = infer_atom(&b.lhs, local, env, is_skirmish, bullet_name, path, span, errors);
            let rhs_ty = infer_atom(&b.rhs, local, env, is_skirmish, bullet_name, path, span, errors);

            if lhs_ty == BuType::Unknown || rhs_ty == BuType::Unknown {
                return BuType::Unknown;
            }
            if lhs_ty != rhs_ty {
                errors.push(terr(path, span, format!(
                    "bullet '{}': binary '{}' has mismatched types: left is {}, right is {}",
                    bullet_name, b.op, lhs_ty.to_rust(), rhs_ty.to_rust()
                )));
                return BuType::Unknown;
            }
            if !lhs_ty.is_numeric() {
                errors.push(terr(path, span, format!(
                    "bullet '{}': binary '{}' requires numeric types, got {}",
                    bullet_name, b.op, lhs_ty.to_rust()
                )));
                return BuType::Unknown;
            }
            lhs_ty
        }

        Expr::Tuple(exprs) => {
            BuType::Tuple(exprs.iter().map(|e| {
                infer_expr(e, local, env, is_skirmish, bullet_name, path, span, errors)
            }).collect())
        }
    }
}

fn infer_atom(
    atom:        &Atom,
    local:       &HashMap<String, BuType>,
    env:         &TypeEnv,
    is_skirmish: bool,
    bullet_name: &str,
    path:        &str,
    span:        Span,
    errors:      &mut Vec<TypeError>,
) -> BuType {
    match atom {
        // Integer literals are untyped — rustc resolves from context
        Atom::Integer(_) => BuType::Unknown,

        Atom::Ident(name) => local.get(name).cloned().unwrap_or(BuType::Unknown),

        Atom::Call { name, args } => {
            if is_skirmish { return BuType::Unknown; }

            let sig = match env.get(name) {
                Some(s) => s.clone(),
                None    => return BuType::Unknown,
            };

            if args.len() != sig.params.len() {
                errors.push(terr(path, span, format!(
                    "bullet '{}': '{}' expects {} argument(s), got {}",
                    bullet_name, name, sig.params.len(), args.len()
                )));
                return sig.returns.clone();
            }

            for (i, (arg, expected_ty)) in args.iter().zip(sig.params.iter()).enumerate() {
                match arg {
                    CallArg::Value(v) => {
                        let actual_ty = local.get(v).cloned().unwrap_or(BuType::Unknown);
                        if actual_ty != BuType::Unknown && !types_compatible(&actual_ty, expected_ty) {
                            errors.push(terr(path, span, format!(
                                "bullet '{}': argument {} to '{}' is {} but expected {}",
                                bullet_name, i + 1, name, actual_ty.to_rust(), expected_ty.to_rust()
                            )));
                        }
                    }
                    CallArg::BulletRef(r) => {
                        let ref_sig = match env.get(r) { Some(s) => s, None => continue };
                        let fn_ty   = build_fn_type(ref_sig);
                        if !types_compatible(&fn_ty, expected_ty) {
                            errors.push(terr(path, span, format!(
                                "bullet '{}': '&{}' has type {} but parameter {} of '{}' expects {}",
                                bullet_name, r, fn_ty.to_rust(), i + 1, name, expected_ty.to_rust()
                            )));
                        }
                    }
                }
            }

            sig.returns.clone()
        }
    }
}

// ── Type utilities ────────────────────────────────────────────────────────────

fn build_fn_type(sig: &BulletSig) -> BuType {
    let params = sig.params.iter().map(|t| t.to_rust()).collect::<Vec<_>>().join(", ");
    BuType::Named(format!("fn({}) -> {}", params, sig.returns.to_rust()))
}

fn normalize(s: &str) -> String { s.split_whitespace().collect() }

fn types_compatible(a: &BuType, b: &BuType) -> bool {
    if a == &BuType::Unknown || b == &BuType::Unknown { return true; }
    match (a, b) {
        (BuType::Named(sa), BuType::Named(sb)) => normalize(sa) == normalize(sb),
        _ => a == b,
    }
}
