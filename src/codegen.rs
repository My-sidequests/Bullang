use crate::ast::*;

// ── Skirmish file → Rust source ───────────────────────────────────────────────

pub fn emit_skirmish(file: &SkirmishFile) -> String {
    let mut out = String::new();

    // Emit imports as `use` statements
    for import in &file.imports {
        out.push_str(&format!("use crate::{}::*;\n", import));
    }
    if !file.imports.is_empty() {
        out.push('\n');
    }

    for bullet in &file.bullets {
        out.push_str(&emit_bullet(bullet, &file.category));
        out.push('\n');
    }

    out
}

// ── Inventory file → mod.rs ───────────────────────────────────────────────────

/// Emits a mod.rs that re-exports everything from child modules.
/// `child_modules` = names of child folders or skirmish files (without extension).
pub fn emit_inventory(
    file:          &InventoryFile,
    child_modules: &[String],
) -> String {
    let mut out = String::new();

    // Declare child modules
    for module in child_modules {
        out.push_str(&format!("pub mod {};\n", module));
    }

    if !child_modules.is_empty() {
        out.push('\n');
    }

    // Re-export the declared exports by searching child modules
    for export in &file.exports {
        // We re-export without specifying which child — let Rust resolve it
        // A future type-resolution pass will make this explicit
        out.push_str(&format!("pub use self::*::{};\n", export));
    }

    out
}

// ── Bullet → Rust function ────────────────────────────────────────────────────

fn emit_bullet(bullet: &Bullet, category: &Category) -> String {
    let mut out = String::new();

    // Doc comment carrying category metadata
    out.push_str(&format!("/// [{}]\n", category.as_str()));

    let vis    = if bullet.exported { "pub " } else { "" };
    let params = emit_params(&bullet.params);
    let ret_ty = bullet.output.ty.to_rust();

    out.push_str(&format!(
        "{}fn {}({}) -> {} {{\n",
        vis, bullet.name, params, ret_ty
    ));

    // Emit each pipe as an immutable let binding
    let pipe_count = bullet.pipes.len();
    for (i, pipe) in bullet.pipes.iter().enumerate() {
        let expr_str = emit_expr(&pipe.expr);
        let is_last  = i == pipe_count - 1;

        if is_last {
            // Final binding is the return expression
            out.push_str(&format!("    let {} = {};\n", pipe.binding, expr_str));
            out.push_str(&format!("    {}\n", pipe.binding));
        } else {
            out.push_str(&format!("    let {} = {};\n", pipe.binding, expr_str));
        }
    }

    out.push_str("}\n");
    out
}

fn emit_params(params: &[Param]) -> String {
    params.iter()
        .map(|p| format!("{}: {}", p.name, p.ty.to_rust()))
        .collect::<Vec<_>>()
        .join(", ")
}

// ── Expression emitters ───────────────────────────────────────────────────────

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
        Atom::Ident(s)          => s.clone(),
        Atom::Integer(n)        => n.to_string(),
        Atom::Call { name, args } => {
            let args_str = args.iter().map(|a| match a {
                CallArg::Value(s)     => s.clone(),
                CallArg::BulletRef(s) => format!("{}", s), // Rust fn pointers are bare names
            }).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, args_str)
        }
    }
}
