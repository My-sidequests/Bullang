use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "to_lower",
    "(s: String)               → String",
    "Lowercase",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("to_lower", params, 1)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.to_lowercase()", p[0]),
        Backend::Python  => format!("{}.lower()", super::py_esc(p[0])),
        Backend::C       => format!("/* to_lower: iterate with tolower() over {} */", p[0]),
        Backend::Cpp     => format!(
            "[&](){{ std::string _s({0}); \
             std::transform(_s.begin(),_s.end(),_s.begin(),::tolower); return _s; }}()",
            p[0]
        ),
        Backend::Go      => format!("strings.ToLower({})", p[0]),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::to_lower' is not available for unknown backend '{}'", kw
        )),
    })
}
