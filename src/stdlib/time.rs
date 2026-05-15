use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "time",
    "()                        → i64",
    "Current Unix timestamp in seconds (seconds since 1970-01-01T00:00:00Z)",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    super::need("time", params, 0)?;
    Ok(match backend {
        // ── Rust ─────────────────────────────────────────────────────────────
        Backend::Rust => "\
std::time::SystemTime::now()\
  .duration_since(std::time::UNIX_EPOCH)\
  .map(|d| d.as_secs() as i64)\
  .unwrap_or(0)".to_string(),

        // ── Python ───────────────────────────────────────────────────────────
        Backend::Python => "int(__import__('time').time())".to_string(),

        // ── C ────────────────────────────────────────────────────────────────
        // time(2) returns time_t; cast to int64_t.  Requires <time.h>.
        Backend::C => "(int64_t)time(NULL)".to_string(),

        // ── C++ ──────────────────────────────────────────────────────────────
        Backend::Cpp => "\
static_cast<int64_t>(\
  std::chrono::duration_cast<std::chrono::seconds>(\
    std::chrono::system_clock::now().time_since_epoch()\
  ).count()\
)".to_string(),

        // ── Go ───────────────────────────────────────────────────────────────
        Backend::Go => "time.Now().Unix()".to_string(),

        Backend::Unknown(kw) => return Err(format!(
            "'builtin::time' is not available for unknown backend '{kw}'"
        )),
    })
}
