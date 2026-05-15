use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "exp",
    "(x: f64)                  → f64",
    "Euler's number raised to the power x (eˣ)",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("exp", params, 1)?;
    let x = p[0];

    Ok(match backend {
        Backend::Rust   => format!("({} as f64).exp()", x),
        Backend::Python => format!("__import__('math').exp({})", super::py_esc(x)),
        Backend::C      => format!("exp((double){})", x),
        Backend::Cpp    => format!("std::exp(static_cast<double>({}))", x),
        Backend::Go     => format!("math.Exp(float64({}))", x),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::exp' is not available for unknown backend '{kw}'"
        )),
    })
}
