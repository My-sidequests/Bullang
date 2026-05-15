use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "args",
    "()                        → Vec[String]",
    "Command-line arguments (argv[1..] — program name excluded)",
);

// C note: Bullang's emitted main() for C projects is expected to populate
// the globals __bu_argc / __bu_argv before any other call.  Using
// builtin::args in a project without a main.bu is a runtime error.

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    super::need("args", params, 0)?;
    Ok(match backend {
        // ── Rust ─────────────────────────────────────────────────────────────
        // std::env::args() includes argv[0]; skip it.
        Backend::Rust => "std::env::args().skip(1).collect::<Vec<String>>()".to_string(),

        // ── Python ───────────────────────────────────────────────────────────
        // sys.argv[0] is the script name; slice it off.
        Backend::Python => "__import__('sys').argv[1:]".to_string(),

        // ── C ────────────────────────────────────────────────────────────────
        // Reads from Bullang-managed globals set by the emitted main().
        // Returns a vec_t* of char* (each string is argv[i]).
        Backend::C => "\
({\
  vec_t *__args = vec_new();\
  extern int   __bu_argc;\
  extern char **__bu_argv;\
  for (int __i = 1; __i < __bu_argc; __i++)\
    vec_push(__args, __bu_argv[__i]);\
  __args;\
})".to_string(),

        // ── C++ ──────────────────────────────────────────────────────────────
        // Same globals, returned as std::vector<std::string>.
        Backend::Cpp => "\
[&]() {\
  extern int   __bu_argc;\
  extern char **__bu_argv;\
  std::vector<std::string> __v;\
  for (int __i = 1; __i < __bu_argc; __i++)\
    __v.emplace_back(__bu_argv[__i]);\
  return __v;\
}()".to_string(),

        // ── Go ───────────────────────────────────────────────────────────────
        // os.Args[0] is the binary name; slice it off.
        Backend::Go => "os.Args[1:]".to_string(),

        Backend::Unknown(kw) => return Err(format!(
            "'builtin::args' is not available for unknown backend '{kw}'"
        )),
    })
}
