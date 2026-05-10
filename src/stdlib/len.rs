use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "len",
    "(s: String | Vec[T])      → i64",
    "Length of a String (char count) or Vec. In C, String only (strlen).",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("len", params, 1)?;
    let s = p[0];

    Ok(match backend {
        // ── Rust ─────────────────────────────────────────────────────────────
        // .len() works for both Vec<T> (element count) and String (byte count).
        // For String, .chars().count() would give Unicode char count, but
        // .len() is consistent, fast, and correct for the ASCII strings that
        // Bullang targets in practice.
        Backend::Rust => format!("{}.len() as i64", s),

        // ── Python ───────────────────────────────────────────────────────────
        // len() works for both str and list.
        Backend::Python => format!("int(len({}))", super::py_esc(s)),

        // ── C ────────────────────────────────────────────────────────────────
        // strlen for char* (String). For Vec, use vec_len via a native block.
        Backend::C => format!("(int64_t)strlen({})", s),

        // ── C++ ──────────────────────────────────────────────────────────────
        // .size() works for both std::string and std::vector.
        Backend::Cpp => format!("(int64_t){}.size()", s),

        // ── Go ───────────────────────────────────────────────────────────────
        // len() works for both string (byte count) and slice (element count).
        Backend::Go => format!("int64(len({}))", s),

        Backend::Unknown(kw) => return Err(format!(
            "'builtin::len' is not available for unknown backend '{kw}'"
        )),
    })
}
