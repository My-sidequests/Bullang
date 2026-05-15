use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "pow",
    "(base: numeric, exp: numeric) → base^exp (integer)",
    "Integer power",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("pow", params, 2)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.pow({} as u32)", p[0], p[1]),
        Backend::Python  => format!(
            "{} ** int({})",
            super::py_esc(p[0]),
            super::py_esc(p[1])
        ),
        Backend::C       => format!(
            "(int64_t)pow((double){}, (double){})",
            p[0], p[1]
        ),
        Backend::Cpp     => format!(
            "(decltype({0}))std::pow((double){0}, (double){1})",
            p[0], p[1]
        ),
        Backend::Go      => format!(
            "func() int64 {{ return int64(math.Round(math.Pow(float64({0}), float64({1})))) }}()",
            p[0], p[1]
        ),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::pow' is not available for unknown backend '{}'", kw
        )),
    })
}
