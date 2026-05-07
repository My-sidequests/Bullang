use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "clamp",
    "(x, min, max)             → x clamped to [min,max]",
    "Clamp to range",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("clamp", params, 3)?;
    Ok(match backend {
        Backend::Rust    => format!("{}.clamp({}, {})", p[0], p[1], p[2]),
        Backend::Python  => format!(
            "max({1}, min({2}, {0}))",
            super::py_esc(p[0]),
            super::py_esc(p[1]),
            super::py_esc(p[2])
        ),
        Backend::C       => format!(
            "({0} < {1} ? {1} : ({0} > {2} ? {2} : {0}))",
            p[0], p[1], p[2]
        ),
        Backend::Cpp     => format!("std::clamp({}, {}, {})", p[0], p[1], p[2]),
        Backend::Go      => format!(
            "func() int64 {{ \
             x,lo,hi:=int64({0}),int64({1}),int64({2}); \
             if x<lo{{return lo}}; if x>hi{{return hi}}; return x \
             }}()",
            p[0], p[1], p[2]
        ),
        Backend::Unknown(kw) => return Err(format!(
            "'builtin::clamp' is not available for unknown backend '{}'", kw
        )),
    })
}
