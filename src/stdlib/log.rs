use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "log",
    "(x: f64)                  → f64",
    "Natural logarithm (ln). Undefined for x ≤ 0 — returns NaN/−∞ per IEEE 754",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("log", params, 1)?;
    let x = p[0];

    Ok(match backend {
        Backend::Rust   => format!("({} as f64).ln()", x),
        Backend::Python => format!("__import__('math').log({})", super::py_esc(x)),
        Backend::C      => format!("log((double){})", x),
        Backend::Cpp    => format!("std::log(static_cast<double>({}))", x),
        Backend::Go     => format!("math.Log(float64({}))", x),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::log' is not available for unknown backend '{kw}'"
        )),
    })
}
