use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "ends_with",
    "(s: String, suffix: String) → bool",
    "Suffix check",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("ends_with", params, 2)?;
    Ok(match backend {
        Backend::Rust    => format!("{0}.ends_with({1}.as_str())", p[0], p[1]),
        Backend::Python  => format!(
            "{}.endswith({})",
            super::py_esc(p[0]),
            super::py_esc(p[1])
        ),
        Backend::C       => format!(
            "(strlen({0}) >= strlen({1}) && \
             strcmp({0} + strlen({0}) - strlen({1}), {1}) == 0)",
            p[0], p[1]
        ),
        Backend::Cpp     => format!(
            "({0}.size() >= {1}.size() && \
             {0}.compare({0}.size()-{1}.size(), {1}.size(), {1}) == 0)",
            p[0], p[1]
        ),
        Backend::Go      => format!("strings.HasSuffix({}, {})", p[0], p[1]),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::ends_with' is not available for unknown backend '{}'", kw
        )),
    })
}
