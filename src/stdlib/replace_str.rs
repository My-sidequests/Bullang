use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "replace_str",
    "(s, from, to: String)     → String",
    "Replace occurrences",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("replace_str", params, 3)?;
    Ok(match backend {
        Backend::Rust    => format!(
            "{0}.replace({1}.as_str(), {2}.as_str())",
            p[0], p[1], p[2]
        ),
        Backend::Python  => format!(
            "{}.replace({}, {})",
            super::py_esc(p[0]),
            super::py_esc(p[1]),
            super::py_esc(p[2])
        ),
        Backend::C       => format!(
            "/* replace_str: implement manually for {} */",
            p[0]
        ),
        Backend::Cpp     => format!(
            "[&](){{ std::string _s={0}; std::size_t pos=0; \
             while((pos=_s.find({1},pos))!=std::string::npos) \
             {{ _s.replace(pos,{1}.size(),{2}); pos+={2}.size(); }} return _s; }}()",
            p[0], p[1], p[2]
        ),
        Backend::Go      => format!(
            "strings.ReplaceAll({}, {}, {})",
            p[0], p[1], p[2]
        ),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::replace_str' is not available for unknown backend '{}'", kw
        )),
    })
}
