use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "tern",
    "(val1, val2, a, b)        → T",
    "Ternary: returns a if val1 == val2, b otherwise — equivalent to val1 == val2 ? a : b",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("tern", params, 4)?;
    let (v1, v2, a, b) = (p[0], p[1], p[2], p[3]);

    Ok(match backend {
        Backend::Rust   => format!("(if {v1} == {v2} {{ {a} }} else {{ {b} }})"),
        Backend::Python => {
            let (v1, v2, a, b) = (
                super::py_esc(v1), super::py_esc(v2),
                super::py_esc(a),  super::py_esc(b),
            );
            format!("({a} if {v1} == {v2} else {b})")
        }
        Backend::C      => format!("(({v1}) == ({v2}) ? ({a}) : ({b}))"),
        Backend::Cpp    => format!("(({v1}) == ({v2}) ? ({a}) : ({b}))"),
        Backend::Go     => format!(
            "func() interface{{}} {{ if {v1} == {v2} {{ return {a} }}; return {b} }}()"
        ),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::tern' is not available for unknown backend '{kw}'"
        )),
    })
}
