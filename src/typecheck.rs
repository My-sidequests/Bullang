//! M3 type checking pass — runs after structural validation.
//!
//! Builds a TypeEnv bottom-up through the folder tree, then checks every
//! call site for arity and type correctness. All errors include file + line:col.
//!
//! Rules (Option A — no implicit coercion):
//!   - Both sides of a binary op must be the same numeric type; result = that type.
//!   - Call argument types must match callee parameter types exactly.
//!   - &bullet_ref type = fn(param_types...) -> return_type; must match param.
//!   - Type of each {binding} is inferred from its producing expression.
//!   - Type of the last binding must match the declared output type.
//!   - Integer literals are untyped (Unknown) — rustc resolves them from context.
//!   - Native (@rust) bullets are trusted verbatim — declared signature is used as-is.
//!   - Input and output types are fully independent — i16 in / Vec<u32> out is valid.

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
    TypeError {
        file:    path.to_string(),
        line:    span.line,
        col:     span.col,
        message: msg.into(),
    }
}

fn tferr(path: &str, msg: impl Into<String>) -> TypeError {
    TypeError { file: path.to_string(), line: 0, col: 0, message: msg.into() }
}

// ── Public entry points ───────────────────────────────────────────────────────

pub fn typecheck_tree(war_root: &Path) -> Vec<TypeError> {
    let mut errors = Vec::new();
    check_folder(war_root, &mut errors);
    errors
}

pub fn typecheck_file(sk: &SkirmishFile, path: &str) -> Vec<TypeError> {
    let mut errors = Vec::new();
    let env = TypeEnv::new();
    for bullet in &sk.bullets {
        errors.extend(check_bullet(bullet, path, &env, sk.rank == Rank::Skirmish));
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

    // Recurse into children first (bottom-up), collecting their type surfaces
    if rank != Rank::War {
        for subdir in collect_subdirs(dir) {
            env.extend(check_folder(&subdir, errors));
        }
    } else {
        // War: recurse but return immediately after (no .bu files)
        for subdir in collect_subdirs(dir) {
            env.extend(check_folder(&subdir, errors));
        }
        return env;
    }

    // Type-check .bu files at this level using the accumulated child env
    for bu_path in collect_bu_files(dir) {
        let file_env = check_bu_file(&bu_path, &env, errors);
        env.extend(file_env);
    }

    env
}

// ── File-level type checking ──────────────────────────────────────────────────

fn check_bu_file(
    path:   &Path,
    env:    &TypeEnv,
    errors: &mut Vec<TypeError>,
) -> TypeEnv {
    let mut exported_env = TypeEnv::new();

    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(_) => return exported_env,
    };

    let bu = match parser::parse_file(&source, false) {
        Ok(f)  => f,
        Err(_) => return exported_env,
    };

    let sk = match bu {
        BuFile::Skirmish(s)  => s,
        BuFile::Inventory(_) => return exported_env,
    };

    let path_str   = path.display().to_string();
    let is_skirmish = sk.rank == Rank::Skirmish;

    for bullet in &sk.bullets {
        let bullet_errors = check_bullet(bullet, &path_str, env, is_skirmish);
        errors.extend(bullet_errors);

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

    // Native bullets: trust the declared signature, do not inspect the body
    let pipes = match &bullet.body {
        BulletBody::Native { .. } => return errors,
        BulletBody::Pipes(p)      => p,
    };

    // Build a local type map from declared params
    let mut local: HashMap<String, BuType> = bullet.params.iter()
        .map(|p| (p.name.clone(), p.ty.clone()))
        .collect();

    let last = pipes.len().saturating_sub(1);

    for (i, pipe) in pipes.iter().enumerate() {
        let expr_type = infer_expr(
            &pipe.expr,
            &local,
            env,
            is_skirmish,
            &bullet.name,
            path,
            pipe.span,
            &mut errors,
        );

        // Last pipe: inferred type must match declared output type
        if i == last {
            let declared = &bullet.output.ty;
            if !types_compatible(&expr_type, declared) {
                errors.push(terr(path, pipe.span, format!(
                    "bullet '{}': last pipe produces {} but declared output is {}",
                    bullet.name,
                    expr_type.to_rust(),
                    declared.to_rust(),
                )));
            }
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
        Expr::Atom(a) => {
            infer_atom(a, local, env, is_skirmish, bullet_name, path, span, errors)
        }

        Expr::BinOp(b) => {
            let lhs_ty = infer_atom(
                &b.lhs, local, env, is_skirmish, bullet_name, path, span, errors
            );
            let rhs_ty = infer_atom(
                &b.rhs, local, env, is_skirmish, bullet_name, path, span, errors
            );

            if lhs_ty == BuType::Unknown || rhs_ty == BuType::Unknown {
                return BuType::Unknown;
            }

            if lhs_ty != rhs_ty {
                errors.push(terr(path, span, format!(
                    "bullet '{}': binary '{}' has mismatched types: \
                     left is {}, right is {}",
                    bullet_name, b.op,
                    lhs_ty.to_rust(), rhs_ty.to_rust()
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
            let types: Vec<BuType> = exprs.iter()
                .map(|e| infer_expr(
                    e, local, env, is_skirmish, bullet_name, path, span, errors
                ))
                .collect();
            BuType::Tuple(types)
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

        // Identifier — look up in local scope
        Atom::Ident(name) => {
            local.get(name).cloned().unwrap_or(BuType::Unknown)
        }

        // Function call
        Atom::Call { name, args } => {
            if is_skirmish {
                return BuType::Unknown;
            }

            let sig = match env.get(name) {
                Some(s) => s.clone(),
                None    => return BuType::Unknown,
            };

            // Arity check
            if args.len() != sig.params.len() {
                errors.push(terr(path, span, format!(
                    "bullet '{}': '{}' expects {} argument(s), got {}",
                    bullet_name, name, sig.params.len(), args.len()
                )));
                return sig.returns.clone();
            }

            // Per-argument type check
            for (i, (arg, expected_ty)) in args.iter().zip(sig.params.iter()).enumerate() {
                match arg {
                    CallArg::Value(v) => {
                        let actual_ty = local.get(v).cloned()
                            .unwrap_or(BuType::Unknown);

                        if actual_ty != BuType::Unknown
                            && !types_compatible(&actual_ty, expected_ty)
                        {
                            errors.push(terr(path, span, format!(
                                "bullet '{}': argument {} to '{}' is {} but expected {}",
                                bullet_name, i + 1, name,
                                actual_ty.to_rust(), expected_ty.to_rust()
                            )));
                        }
                    }

                    CallArg::BulletRef(r) => {
                        let ref_sig = match env.get(r) {
                            Some(s) => s,
                            None    => continue,
                        };
                        let fn_ty = build_fn_type(ref_sig);
                        if !types_compatible(&fn_ty, expected_ty) {
                            errors.push(terr(path, span, format!(
                                "bullet '{}': '&{}' has type {} \
                                 but parameter {} of '{}' expects {}",
                                bullet_name, r,
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
    let params = sig.params.iter()
        .map(|t| t.to_rust())
        .collect::<Vec<_>>()
        .join(", ");
    BuType::Named(format!("fn({}) -> {}", params, sig.returns.to_rust()))
}

fn normalize_type_str(s: &str) -> String {
    s.split_whitespace().collect::<String>()
}

fn types_compatible(a: &BuType, b: &BuType) -> bool {
    if a == &BuType::Unknown || b == &BuType::Unknown {
        return true;
    }
    match (a, b) {
        (BuType::Named(sa), BuType::Named(sb)) => {
            normalize_type_str(sa) == normalize_type_str(sb)
        }
        _ => a == b,
    }
}
