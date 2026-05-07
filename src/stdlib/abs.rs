use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "abs",
    "(x: numeric)             → |x|",
    "Absolute value",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("abs", params, 1)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.abs()", p[0]),
        Backend::Python  => format!("abs({})", super::py_esc(p[0])),
        Backend::C       => format!("abs({})", p[0]),
        Backend::Cpp     => format!("std::abs({})", p[0]),
        Backend::Go      => format!(
            "func() int64 {{ if {0} < 0 {{ return int64(-{0}) }}; return int64({0}) }}()",
            p[0]
        ),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::abs' is not available for unknown backend '{}'", kw
        )),
    })
}
