use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "trim",
    "(s: String)               → String",
    "Strip whitespace",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("trim", params, 1)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.trim().to_owned()", p[0]),
        Backend::Python  => format!("{}.strip()", super::py_esc(p[0])),
        Backend::C       => format!("/* trim: implement manually for {} */", p[0]),
        Backend::Cpp     => format!(
            "[&](){{ std::string _s({0}); \
             _s.erase(0,_s.find_first_not_of(\" \\t\\n\\r\")); \
             _s.erase(_s.find_last_not_of(\" \\t\\n\\r\")+1); return _s; }}()",
            p[0]
        ),
        Backend::Go      => format!("strings.TrimSpace({})", p[0]),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::trim' is not available for unknown backend '{}'", kw
        )),
    })
}
