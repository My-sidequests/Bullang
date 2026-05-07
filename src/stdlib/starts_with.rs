use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "starts_with",
    "(s: String, prefix: String) → bool",
    "Prefix check",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("starts_with", params, 2)?;
    Ok(match backend {
        Backend::Rust    => format!("{0}.starts_with({1}.as_str())", p[0], p[1]),
        Backend::Python  => format!(
            "{}.startswith({})",
            super::py_esc(p[0]),
            super::py_esc(p[1])
        ),
        Backend::C       => format!(
            "(strncmp({0}, {1}, strlen({1})) == 0)",
            p[0], p[1]
        ),
        Backend::Cpp     => format!("{0}.rfind({1}, 0) == 0", p[0], p[1]),
        Backend::Go      => format!("strings.HasPrefix({}, {})", p[0], p[1]),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::starts_with' is not available for unknown backend '{}'", kw
        )),
    })
}
