use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "to_string",
    "(x: numeric)              → String",
    "Convert to string",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("to_string", params, 1)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.to_string()", p[0]),
        Backend::Python  => format!("str({})", super::py_esc(p[0])),
        Backend::C       => format!("/* to_string: use sprintf for {} */", p[0]),
        Backend::Cpp     => format!("std::to_string({})", p[0]),
        Backend::Go      => format!("fmt.Sprintf(\"%v\", {})", p[0]),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::to_string' is not available for unknown backend '{}'", kw
        )),
    })
}
