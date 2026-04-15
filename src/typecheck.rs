//! Type checking pass.

use std::collections::HashMap;
use std::path::Path;
use std::fs;

use crate::ast::*;
use crate::parser;
use crate::validator::{collect_subdirs, read_inventory};

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

pub fn typecheck_tree(root: &Path) -> Vec<TypeError> {
    let mut errors = Vec::new();
    check_folder(root, &mut errors);
    errors
}

pub fn typecheck_file(sf: &SourceFile, path: &str) -> Vec<TypeError> {
    let mut errors = Vec::new();
    for func in &sf.bullets {
        errors.extend(check_function(func, path, &TypeEnv::new(), true));
    }
    errors
}

// ── Folder-level type checking ────────────────────────────────────────────────

fn check_folder(dir: &Path, errors: &mut Vec<TypeError>) -> TypeEnv {
    let inv = match read_inventory(dir) {
        Ok(i)  => i,
        Err(_) => return TypeEnv::new(),
    };

    let mut env = TypeEnv::new();

    if inv.rank == Rank::War {
        for subdir in collect_subdirs(dir) {
            env.extend(check_folder(&subdir, errors));
        }
        return env;
    }

    if inv.rank.has_sub_folders() {
        for subdir in collect_subdirs(dir) {
            env.extend(check_folder(&subdir, errors));
        }
    }

    let is_skirmish = inv.rank == Rank::Skirmish;
    for entry in &inv.entries {
        let bu_path  = dir.join(format!("{}.bu", entry.file));
        let file_env = check_source_file(&bu_path, &env, is_skirmish, errors);
        env.extend(file_env);
    }

    env
}

// ── File-level type checking ──────────────────────────────────────────────────

fn check_source_file(
    path:        &Path,
    env:         &TypeEnv,
    is_skirmish: bool,
    errors:      &mut Vec<TypeError>,
) -> TypeEnv {
    let mut file_env = TypeEnv::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(_) => return file_env,
    };

    let sf = match parser::parse_file(&source, false) {
        Ok(BuFile::Source(s)) => s,
        _                     => return file_env,
    };

    let path_str = path.display().to_string();

    for func in &sf.bullets {
        errors.extend(check_function(func, &path_str, env, is_skirmish));
        file_env.insert(func.name.clone(), BulletSig {
            params:  func.params.iter().map(|p| p.ty.clone()).collect(),
            returns: func.output.ty.clone(),
        });
    }

    file_env
}

// ── Function-level type checking ──────────────────────────────────────────────

fn check_function(
    func:        &Bullet,
    path:        &str,
    env:         &TypeEnv,
    is_skirmish: bool,
) -> Vec<TypeError> {
    let bullets = match &func.body {
        BulletBody::Native { .. }  => return vec![],
        BulletBody::Builtin(_)     => return vec![], // stdlib owns the type contract
        BulletBody::Pipes(p)       => p,
    };

    let mut errors = Vec::new();
    let mut local: HashMap<String, BuType> = func.params.iter()
        .map(|p| (p.name.clone(), p.ty.clone()))
        .collect();

    let last = bullets.len().saturating_sub(1);

    for (i, bullet) in bullets.iter().enumerate() {
        let expr_type = infer_expr(
            &bullet.expr, &local, env, is_skirmish,
            &func.name, path, bullet.span, &mut errors,
        );

        // Last bullet: inferred type must match declared output type
        if i == last && !types_compatible(&expr_type, &func.output.ty) {
            errors.push(terr(path, bullet.span, format!(
                "Function '{}': last bullet produces {} but declared output is {}.",
                func.name, expr_type.to_rust(), func.output.ty.to_rust()
            )));
        }

        local.insert(bullet.binding.clone(), expr_type);
    }

    errors
}

// ── Type inference ────────────────────────────────────────────────────────────

fn infer_expr(
    expr:        &Expr,
    local:       &HashMap<String, BuType>,
    env:         &TypeEnv,
    is_skirmish: bool,
    func_name:   &str,
    path:        &str,
    span:        Span,
    errors:      &mut Vec<TypeError>,
) -> BuType {
    match expr {
        Expr::Atom(a) => infer_atom(a, local, env, is_skirmish, func_name, path, span, errors),

        Expr::BinOp(b) => {
            let lhs_ty = infer_atom(&b.lhs, local, env, is_skirmish, func_name, path, span, errors);
            let rhs_ty = infer_atom(&b.rhs, local, env, is_skirmish, func_name, path, span, errors);

            if lhs_ty == BuType::Unknown || rhs_ty == BuType::Unknown {
                return BuType::Unknown;
            }
            if lhs_ty != rhs_ty {
                errors.push(terr(path, span, format!(
                    "Function '{}': operator '{}' requires both sides to be the same type \
                     (left: {}, right: {}).",
                    func_name, b.op, lhs_ty.to_rust(), rhs_ty.to_rust()
                )));
                return BuType::Unknown;
            }
            if !lhs_ty.is_numeric() {
                errors.push(terr(path, span, format!(
                    "Function '{}': operator '{}' requires a numeric type, got {}.",
                    func_name, b.op, lhs_ty.to_rust()
                )));
                return BuType::Unknown;
            }
            lhs_ty
        }

        Expr::Tuple(exprs) => {
            BuType::Tuple(exprs.iter().map(|e| {
                infer_expr(e, local, env, is_skirmish, func_name, path, span, errors)
            }).collect())
        }
    }
}

fn infer_atom(
    atom:        &Atom,
    local:       &HashMap<String, BuType>,
    env:         &TypeEnv,
    is_skirmish: bool,
    func_name:   &str,
    path:        &str,
    span:        Span,
    errors:      &mut Vec<TypeError>,
) -> BuType {
    match atom {
        Atom::Integer(_)  => BuType::Unknown,
        Atom::Ident(name) => local.get(name).cloned().unwrap_or(BuType::Unknown),

        Atom::Call { name, args } => {
            if is_skirmish { return BuType::Unknown; }

            let sig = match env.get(name) {
                Some(s) => s.clone(),
                None    => return BuType::Unknown,
            };

            if args.len() != sig.params.len() {
                errors.push(terr(path, span, format!(
                    "Function '{}': '{}' expects {} argument(s) but received {}.",
                    func_name, name, sig.params.len(), args.len()
                )));
                return sig.returns.clone();
            }

            for (i, (arg, expected_ty)) in args.iter().zip(sig.params.iter()).enumerate() {
                match arg {
                    CallArg::Value(v) => {
                        let actual_ty = local.get(v).cloned().unwrap_or(BuType::Unknown);
                        if actual_ty != BuType::Unknown && !types_compatible(&actual_ty, expected_ty) {
                            errors.push(terr(path, span, format!(
                                "Function '{}': argument {} passed to '{}' is {} but {} was expected.",
                                func_name, i + 1, name,
                                actual_ty.to_rust(), expected_ty.to_rust()
                            )));
                        }
                    }
                    CallArg::BulletRef(r) => {
                        let ref_sig = match env.get(r) { Some(s) => s, None => continue };
                        let fn_ty   = build_fn_type(ref_sig);
                        if !types_compatible(&fn_ty, expected_ty) {
                            errors.push(terr(path, span, format!(
                                "Function '{}': '&{}' has type {} but argument {} of '{}' expects {}.",
                                func_name, r,
                                fn_ty.to_rust(), i + 1, name,
                                expected_ty.to_rust()
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
