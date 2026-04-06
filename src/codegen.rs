use crate::ast::*;

// ── Skirmish file → Rust source ───────────────────────────────────────────────

pub fn emit_skirmish(file: &SkirmishFile) -> String {
    let mut out = String::new();

    // Every generated .rs file pulls in the full crate re-export surface.
    // Called bullets from any rank are resolved through the lib.rs glob chain.
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use crate::*;\n\n");

    for bullet in &file.bullets {
        out.push_str(&emit_bullet(bullet, &file.category));
        out.push('\n');
    }

    out
}

// ── Inventory → mod.rs ────────────────────────────────────────────────────────

/// Emits a mod.rs for a folder level.
/// Declares all child modules then re-exports everything from each via glob.
/// Each child module carries its own `pub` declarations — a single glob per
/// child surfaces the entire export surface cleanly without enumerating names.
pub fn emit_mod_rs(child_modules: &[String], _exports: &[String]) -> String {
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

/// Emits lib.rs for the war root — same pattern as mod.rs.
pub fn emit_lib_rs(child_modules: &[String], _exports: &[String]) -> String {
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

/// Emits the Cargo.toml for the generated crate.
pub fn emit_cargo_toml(crate_name: &str) -> String {
    format!(
        "[package]\nname    = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        crate_name
    )
}

// ── Bullet → Rust function ────────────────────────────────────────────────────

fn emit_bullet(bullet: &Bullet, category: &Category) -> String {
    let mut out = String::new();

    out.push_str(&format!("/// [{}]\n", category.as_str()));

    let vis    = if bullet.exported { "pub " } else { "" };
    let params = emit_params(&bullet.params);
    let ret_ty = bullet.output.ty.to_rust();

    out.push_str(&format!(
        "{}fn {}({}) -> {} {{\n",
        vis, bullet.name, params, ret_ty
    ));

    match &bullet.body {
        BulletBody::Pipes(pipes) => {
            emit_pipes(&mut out, pipes);
        }
        BulletBody::Native { code, .. } => {
            for line in code.lines() {
                if line.trim().is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&format!("    {}\n", line));
                }
            }
        }
    }

    out.push_str("}\n");
    out
}

fn emit_pipes(out: &mut String, pipes: &[Pipe]) {
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

fn emit_params(params: &[Param]) -> String {
    params.iter()
        .map(|p| format!("{}: {}", p.name, p.ty.to_rust()))
        .collect::<Vec<_>>()
        .join(", ")
}

// ── Expressions ───────────────────────────────────────────────────────────────

fn emit_expr(expr: &Expr) -> String {
    match expr {
        Expr::Atom(a)      => emit_atom(a),
        Expr::BinOp(b)     => format!(
            "{} {} {}",
            emit_atom(&b.lhs), b.op, emit_atom(&b.rhs)
        ),
        Expr::Tuple(exprs) => format!(
            "({})",
            exprs.iter().map(emit_expr).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn emit_atom(atom: &Atom) -> String {
    match atom {
        Atom::Ident(s)            => s.clone(),
        Atom::Integer(n)          => n.to_string(),
        Atom::Call { name, args } => {
            let args_str = args.iter()
                .map(|a| match a {
                    CallArg::Value(s)     => s.clone(),
                    CallArg::BulletRef(s) => s.clone(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", name, args_str)
        }
    }
}
