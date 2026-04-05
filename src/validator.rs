use std::path::Path;
use std::collections::{HashMap, HashSet};
use crate::ast::*;

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

// ── Skirmish file validation ──────────────────────────────────────────────────

pub fn validate_skirmish(
    file:            &SkirmishFile,
    path:            &str,
    inventory_names: &HashSet<String>, // names available from imported inventory
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Rule: must declare #rank: skirmish
    if file.rank != Rank::Skirmish {
        errors.push(err(path, format!(
            "expected #rank: skirmish, found: {}",
            file.rank.name()
        )));
    }

    // Rule: max 5 bullets
    if file.bullets.len() > 5 {
        errors.push(err(path, format!(
            "skirmish files may contain at most 5 bullets, found {}",
            file.bullets.len()
        )));
    }

    // Rule: every exported name must correspond to a bullet
    for export in &file.exports {
        if !file.bullets.iter().any(|b| &b.name == export) {
            errors.push(err(path, format!(
                "#export '{}' has no matching bullet",
                export
            )));
        }
    }

    for bullet in &file.bullets {
        errors.extend(validate_bullet(bullet, path, inventory_names));
    }

    errors
}

// ── Bullet validation ─────────────────────────────────────────────────────────

fn validate_bullet(
    bullet:          &Bullet,
    path:            &str,
    inventory_names: &HashSet<String>,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let param_names: HashSet<&str> = bullet.params.iter()
        .map(|p| p.name.as_str())
        .collect();

    // Track bindings produced so far in this bullet's pipe chain
    let mut bound: HashSet<String> = HashSet::new();
    // Track which bindings are consumed
    let mut consumed: HashSet<String> = HashSet::new();

    let pipe_count = bullet.pipes.len();

    for (i, pipe) in bullet.pipes.iter().enumerate() {
        let is_last = i == pipe_count - 1;

        // Rule: pipe inputs must be param names or earlier bindings
        for input in &pipe.inputs {
            let is_param   = param_names.contains(input.as_str());
            let is_binding = bound.contains(input.as_str());
            if !is_param && !is_binding {
                errors.push(err(path, format!(
                    "bullet '{}' pipe {}: input '{}' is not a declared param or earlier binding",
                    bullet.name, i + 1, input
                )));
            } else {
                consumed.insert(input.clone());
            }
        }

        // Rule: calls may only reference names in the inventory
        validate_expr_calls(&pipe.expr, bullet, path, inventory_names, &mut errors);

        // Rule: binding must not already exist (no reassignment)
        if bound.contains(&pipe.binding) {
            errors.push(err(path, format!(
                "bullet '{}': binding '{{{}}}' is assigned more than once",
                bullet.name, pipe.binding
            )));
        }

        // Rule: last pipe must assign to the declared output name
        if is_last && pipe.binding != bullet.output.name {
            errors.push(err(path, format!(
                "bullet '{}': last pipe binds to '{{{}}}' but declared output is '{{{}}}'",
                bullet.name, pipe.binding, bullet.output.name
            )));
        }

        bound.insert(pipe.binding.clone());
    }

    // Warning-level: intermediate bindings that are never consumed
    for b in &bound {
        if b != &bullet.output.name && !consumed.contains(b) {
            errors.push(err(path, format!(
                "bullet '{}': binding '{{{}}}' is produced but never consumed (dead intermediate)",
                bullet.name, b
            )));
        }
    }

    errors
}

fn validate_expr_calls(
    expr:            &Expr,
    bullet:          &Bullet,
    path:            &str,
    inventory_names: &HashSet<String>,
    errors:          &mut Vec<ValidationError>,
) {
    match expr {
        Expr::Atom(atom)    => validate_atom_calls(atom, bullet, path, inventory_names, errors),
        Expr::BinOp(binop)  => {
            validate_atom_calls(&binop.lhs, bullet, path, inventory_names, errors);
            validate_atom_calls(&binop.rhs, bullet, path, inventory_names, errors);
        }
        Expr::Tuple(exprs)  => {
            for e in exprs {
                validate_expr_calls(e, bullet, path, inventory_names, errors);
            }
        }
    }
}

fn validate_atom_calls(
    atom:            &Atom,
    bullet:          &Bullet,
    path:            &str,
    inventory_names: &HashSet<String>,
    errors:          &mut Vec<ValidationError>,
) {
    if let Atom::Call { name, args } = atom {
        // Rule: called bullet must be in the imported inventory
        if !inventory_names.contains(name.as_str()) {
            errors.push(err(path, format!(
                "bullet '{}': calls '{}' which is not in the imported inventory",
                bullet.name, name
            )));
        }

        // Rule: &bullet_ref targets must also be in the inventory
        for arg in args {
            if let CallArg::BulletRef(ref_name) = arg {
                if !inventory_names.contains(ref_name.as_str()) {
                    errors.push(err(path, format!(
                        "bullet '{}': references '&{}' which is not in the imported inventory",
                        bullet.name, ref_name
                    )));
                }
            }
        }
    }
}

// ── Rank depth validation ─────────────────────────────────────────────────────

pub fn validate_rank_depth(
    declared_rank: &Rank,
    actual_path:   &Path,
    war_root:      &Path,
) -> Option<ValidationError> {
    let depth = actual_path
        .strip_prefix(war_root)
        .map(|p| p.components().count().saturating_sub(1))
        .unwrap_or(0);

    if depth != declared_rank.expected_depth() {
        Some(err(
            &actual_path.display().to_string(),
            format!(
                "#rank: {} expects depth {} from war root, but file is at depth {}",
                declared_rank.name(),
                declared_rank.expected_depth(),
                depth
            ),
        ))
    } else {
        None
    }
}

// ── Folder child-count validation ─────────────────────────────────────────────

pub fn validate_folder_counts(
    dir:      &Path,
    rank:     &Rank,
    war_root: &Path,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let child_rank = match rank.child_rank() {
        Some(r) => r,
        None    => return errors,
    };

    let children: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let p = e.path();
            if child_rank == Rank::Skirmish {
                p.extension().map(|x| x == "bu").unwrap_or(false)
                    && p.file_name().map(|x| x != "inventory.bu").unwrap_or(false)
            } else {
                p.is_dir()
            }
        })
        .collect();

    if children.len() > 5 {
        errors.push(err(
            &dir.display().to_string(),
            format!(
                "{} folder may contain at most 5 {} children, found {}",
                rank.name(), child_rank.name(), children.len()
            ),
        ));
    }

    if child_rank != Rank::Skirmish {
        for child in children {
            errors.extend(
                validate_folder_counts(&child.path(), &child_rank, war_root)
            );
        }
    }

    errors
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn err(file: &str, message: String) -> ValidationError {
    ValidationError { file: file.to_string(), message }
}
