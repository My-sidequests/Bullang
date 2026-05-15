use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "parse_i64",
    "(s: String)               → i64 (or error)",
    "Parse string to integer",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("parse_i64", params, 1)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.trim().parse::<i64>().unwrap_or(0)", p[0]),
        Backend::Python  => format!("int({}.strip())", super::py_esc(p[0])),
        Backend::C       => format!("strtoll({}, NULL, 10)", p[0]),
        Backend::Cpp     => format!("std::stoll({})", p[0]),
        Backend::Go      => format!(
            "func() int64 {{ \
             n, _ := strconv.ParseInt(strings.TrimSpace({}), 10, 64); return n \
             }}()",
            p[0]
        ),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::parse_i64' is not available for unknown backend '{}'", kw
        )),
    })
}
