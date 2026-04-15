//! Code generation — AST → Rust source.

use crate::ast::*;
use crate::stdlib;

// ── Source file → Rust ────────────────────────────────────────────────────────

pub fn emit_source(file: &SourceFile) -> String {
    let mut out = String::new();
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use crate::*;\n\n");
    for func in &file.bullets {
        out.push_str(&emit_function(func, &Backend::Rust));
        out.push('\n');
    }
    out
}

// ── main.bu → main.rs ─────────────────────────────────────────────────────────

/// Emits src/main.rs from the parsed main.bu.
/// The main function gets `fn main()` — no pub, no return type.
/// All other functions in main.bu (helpers) get `fn` but not `pub`.
pub fn emit_main(file: &SourceFile, crate_name: &str) -> String {
    let mut out = String::new();

    // Import the library crate so all transpiled functions are in scope
    out.push_str(&format!("use {}::*;\n\n", crate_name));

    for func in &file.bullets {
        if func.name == "main" {
            out.push_str(&emit_main_function(func));
        } else {
            out.push_str(&emit_function(func, &Backend::Rust));
        }
        out.push('\n');
    }

    out
}

/// Emits Cargo.toml with both a [[bin]] and [lib] section when main.rs exists.
pub fn emit_cargo_toml_with_main(crate_name: &str) -> String {
    format!(
        "[package]\n\
         name    = \"{name}\"\n\
         version = \"0.1.0\"\n\
         edition = \"2021\"\n\n\
         [[bin]]\n\
         name = \"{name}\"\n\
         path = \"src/main.rs\"\n\n\
         [lib]\n\
         name = \"{name}\"\n\
         path = \"src/lib.rs\"\n\n\
         [dependencies]\n",
        name = crate_name
    )
}

/// Emits Cargo.toml as a library-only crate (no main.bu present).
pub fn emit_cargo_toml(crate_name: &str) -> String {
    format!(
        "[package]\nname    = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        crate_name
    )
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

// ── Function emitters ─────────────────────────────────────────────────────────

/// Emit a regular function. All are `pub` since there is no private code in Bullang.
fn emit_function(func: &Bullet, backend: &Backend) -> String {
    let mut out = String::new();

    let params = func.params.iter()
        .map(|p| format!("{}: {}", p.name, p.ty.to_rust()))
        .collect::<Vec<_>>().join(", ");
    let ret_ty = func.output.ty.to_rust();

    out.push_str(&format!("pub fn {}({}) -> {} {{\n", func.name, params, ret_ty));
    emit_body(&mut out, &func.body, &func.params, backend);
    out.push_str("}\n");
    out
}

/// Emit the `main` function: no pub, no return type annotation.
fn emit_main_function(func: &Bullet) -> String {
    let mut out = String::new();

    // main() takes no arguments in Rust
    out.push_str("fn main() {\n");
    emit_body(&mut out, &func.body, &func.params, &Backend::Rust);
    out.push_str("}\n");
    out
}

fn emit_body(out: &mut String, body: &BulletBody, params: &[Param], backend: &Backend) {
    match body {
        BulletBody::Pipes(pipes) => {
            let last = pipes.len().saturating_sub(1);
            for (i, pipe) in pipes.iter().enumerate() {
                let expr_str = emit_expr(&pipe.expr);
                if i == last {
                    out.push_str(&format!("    let {} = {};\n", pipe.binding, expr_str));
                    let binding = &pipe.binding;
                    out.push_str(&format!("    {}\n", binding));
                } else {
                    out.push_str(&format!("    let {} = {};\n", pipe.binding, expr_str));
                }
            }
        }
        BulletBody::Native { backend, code } => {
            match backend {
                Backend::Unknown(kw) => {
                    out.push_str(&format!(
                        "    compile_error!(\"\'@{}\' is not a supported backend\")\n",
                        kw
                    ));
                }
                _ => {
                    for line in code.lines() {
                        if line.trim().is_empty() { out.push('\n'); }
                        else { out.push_str(&format!("    {}\n", line)); }
                    }
                }
            }
        }
        BulletBody::Builtin(name) => {
            match stdlib::emit_builtin(name, params, backend) {
                Ok(code) => out.push_str(&format!("    {}\n", code)),
                Err(e)   => out.push_str(&format!("    compile_error!(\"{}\")\n", e)),
            }
        }
    }
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
