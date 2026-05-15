use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "to_upper",
    "(s: String)               → String",
    "Uppercase",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("to_upper", params, 1)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.to_uppercase()", p[0]),
        Backend::Python  => format!("{}.upper()", super::py_esc(p[0])),
        Backend::C       => format!("/* to_upper: iterate with toupper() over {} */", p[0]),
        Backend::Cpp     => format!(
            "[&](){{ std::string _s({0}); \
             std::transform(_s.begin(),_s.end(),_s.begin(),::toupper); return _s; }}()",
            p[0]
        ),
        Backend::Go      => format!("strings.ToUpper({})", p[0]),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::to_upper' is not available for unknown backend '{}'", kw
        )),
    })
}
