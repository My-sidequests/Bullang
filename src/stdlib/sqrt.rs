use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "sqrt",
    "(x: f64)                  → √x",
    "Square root",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("sqrt", params, 1)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.sqrt()", p[0]),
        Backend::Python  => format!("__import__('math').sqrt({})", super::py_esc(p[0])),
        Backend::C       => format!("sqrt((double){})", p[0]),
        Backend::Cpp     => format!("std::sqrt({})", p[0]),
        Backend::Go      => format!("math.Sqrt(float64({}))", p[0]),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::sqrt' is not available for unknown backend '{}'", kw
        )),
    })
}
