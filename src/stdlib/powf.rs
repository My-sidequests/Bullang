use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "powf",
    "(base: f64, exp: f64)     → base^exp (float)",
    "Float power",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("powf", params, 2)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.powf({})", p[0], p[1]),
        Backend::Python  => format!(
            "{} ** {}",
            super::py_esc(p[0]),
            super::py_esc(p[1])
        ),
        Backend::C       => format!("pow({}, {})", p[0], p[1]),
        Backend::Cpp     => format!("std::pow({}, {})", p[0], p[1]),
        Backend::Go      => format!("math.Pow({}, {})", p[0], p[1]),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::powf' is not available for unknown backend '{}'", kw
        )),
    })
}
