//! Code generation — AST → Rust source.

use crate::ast::*;

// ── Source file → Rust ────────────────────────────────────────────────────────

pub fn emit_source(file: &SourceFile) -> String {
    let mut out = String::new();
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use crate::*;\n\n");
    for bullet in &file.bullets {
        out.push_str(&emit_bullet(bullet));
        out.push('\n');
    }
    out
}

// ── Module files ──────────────────────────────────────────────────────────────

pub fn emit_mod_rs(child_modules: &[String]) -> String {
    let mut out = String::new();
    for module in child_modules {
        out.push_str(&format!("pub mod {};\n", module));
    }
    if !child_modules.is_empty() {
        out.push('\n');
        for module in child_modules {
            out.push_str(&format!("pub use {}::*;\n", module));
        }
    }
    out
}

pub fn emit_lib_rs(child_modules: &[String]) -> String {
    let mut out = String::new();
    out.push_str("#![allow(unused_imports)]\n\n");
    for module in child_modules {
        out.push_str(&format!("pub mod {};\n", module));
    }
    if !child_modules.is_empty() {
        out.push('\n');
        for module in child_modules {
            out.push_str(&format!("pub use {}::*;\n", module));
        }
    }
    out
}

pub fn emit_cargo_toml(crate_name: &str) -> String {
    format!(
        "[package]\nname    = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        crate_name
    )
}

// ── Bullet → Rust function ────────────────────────────────────────────────────

fn emit_bullet(bullet: &Bullet) -> String {
    let mut out = String::new();

    let params = bullet.params.iter()
        .map(|p| format!("{}: {}", p.name, p.ty.to_rust()))
        .collect::<Vec<_>>().join(", ");
    let ret_ty = bullet.output.ty.to_rust();

    // All bullets are always public
    out.push_str(&format!("pub fn {}({}) -> {} {{\n", bullet.name, params, ret_ty));

    match &bullet.body {
        BulletBody::Pipes(pipes) => {
            let last = pipes.len().saturating_sub(1);
            for (i, pipe) in pipes.iter().enumerate() {
                let expr_str = emit_expr(&pipe.expr);
                if i == last {
                    out.push_str(&format!("    let {} = {};\n", pipe.binding, expr_str));
                    out.push_str(&format!("    {}\n", pipe.binding));
                } else {
                    out.push_str(&format!("    let {} = {};\n", pipe.binding, expr_str));
                }
            }
        }
        BulletBody::Native { code, .. } => {
            for line in code.lines() {
                if line.trim().is_empty() { out.push('\n'); }
                else { out.push_str(&format!("    {}\n", line)); }
            }
        }
    }

    out.push_str("}\n");
    out
}

// ── Expression emitters ───────────────────────────────────────────────────────

fn emit_expr(expr: &Expr) -> String {
    match expr {
        Expr::Atom(a)      => emit_atom(a),
        Expr::BinOp(b)     => format!("{} {} {}", emit_atom(&b.lhs), b.op, emit_atom(&b.rhs)),
        Expr::Tuple(exprs) => format!(
            "({})", exprs.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn emit_atom(atom: &Atom) -> String {
    match atom {
        Atom::Ident(s)            => s.clone(),
        Atom::Integer(n)          => n.to_string(),
        Atom::Call { name, args } => {
            let args_str = args.iter().map(|a| match a {
                CallArg::Value(s)     => s.clone(),
                CallArg::BulletRef(s) => s.clone(),
            }).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, args_str)
        }
    }
}
