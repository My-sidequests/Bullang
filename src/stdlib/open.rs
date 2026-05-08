use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "open",
    "(path: String, mode: String) → i32",
    "Open a file — mode: \"r\" | \"w\" | \"a\" | \"rw\". Returns fd (i32), -1 on error",
);

// Mode string convention (same across all backends):
//   "r"  → read-only
//   "w"  → write-only, create/truncate
//   "a"  → write-only, create/append
//   "rw" → read-write, create if absent
// Any unrecognised mode falls back to read-only (Rust/Go) or O_RDONLY (C/C++).

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("open", params, 2)?;
    let (path, mode) = (p[0], p[1]);

    Ok(match backend {
        // ── Rust ─────────────────────────────────────────────────────────────
        // Uses std::fs::OpenOptions to build the right flags, then converts to
        // a raw fd via IntoRawFd.  Returns -1 on any error.
        Backend::Rust => format!(
            "{{\
               use std::os::unix::io::IntoRawFd;\
               let __mode: &str = {mode}.as_str();\
               let mut __oo = std::fs::OpenOptions::new();\
               match __mode {{\
                 \"r\"  => {{ __oo.read(true); }}\
                 \"w\"  => {{ __oo.write(true).create(true).truncate(true); }}\
                 \"a\"  => {{ __oo.append(true).create(true); }}\
                 \"rw\" => {{ __oo.read(true).write(true).create(true); }}\
                 _     => {{ __oo.read(true); }}\
               }}\
               __oo.open({path}.as_str()).map(|f| f.into_raw_fd()).unwrap_or(-1)\
             }}"
        ),

        // ── Python ───────────────────────────────────────────────────────────
        // os.open returns an integer fd directly.  Mode string → O_* flags.
        Backend::Python => {
            let path = super::py_esc(path);
            let mode = super::py_esc(mode);
            format!(
                "(lambda __p, __m: \
                   (lambda __os: \
                     __os.open(__p, {{\
                       'r':  __os.O_RDONLY,\
                       'w':  __os.O_WRONLY | __os.O_CREAT | __os.O_TRUNC,\
                       'a':  __os.O_WRONLY | __os.O_CREAT | __os.O_APPEND,\
                       'rw': __os.O_RDWR  | __os.O_CREAT,\
                     }}.get(__m, __os.O_RDONLY), 0o644)\
                   )(__import__('os'))\
                 )({path}, {mode})"
            )
        }

        // ── C ────────────────────────────────────────────────────────────────
        // POSIX open(2). Requires <fcntl.h> and <sys/stat.h>.
        Backend::C => format!(
            "({{ \
               int __flags; \
               if      (strcmp({mode}, \"r\" ) == 0) __flags = O_RDONLY; \
               else if (strcmp({mode}, \"w\" ) == 0) __flags = O_WRONLY | O_CREAT | O_TRUNC; \
               else if (strcmp({mode}, \"a\" ) == 0) __flags = O_WRONLY | O_CREAT | O_APPEND; \
               else if (strcmp({mode}, \"rw\") == 0) __flags = O_RDWR  | O_CREAT; \
               else                                  __flags = O_RDONLY; \
               (int32_t)open({path}, __flags, 0644); \
             }})"
        ),

        // ── C++ ──────────────────────────────────────────────────────────────
        // Same POSIX open(2), wrapped in an immediately-invoked lambda.
        Backend::Cpp => format!(
            "[&]() -> int32_t {{ \
               int __flags; \
               if      ({mode} == \"r\" ) __flags = O_RDONLY; \
               else if ({mode} == \"w\" ) __flags = O_WRONLY | O_CREAT | O_TRUNC; \
               else if ({mode} == \"a\" ) __flags = O_WRONLY | O_CREAT | O_APPEND; \
               else if ({mode} == \"rw\") __flags = O_RDWR  | O_CREAT; \
               else                      __flags = O_RDONLY; \
               return static_cast<int32_t>(open({path}.c_str(), __flags, 0644)); \
             }}()"
        ),

        // ── Go ───────────────────────────────────────────────────────────────
        // os.OpenFile; returns the raw fd as i32, or -1 on error.
        Backend::Go => format!(
            "func() int32 {{ \
               var __flag int; \
               switch {mode} {{ \
               case \"r\":  __flag = os.O_RDONLY; \
               case \"w\":  __flag = os.O_WRONLY | os.O_CREATE | os.O_TRUNC; \
               case \"a\":  __flag = os.O_WRONLY | os.O_CREATE | os.O_APPEND; \
               case \"rw\": __flag = os.O_RDWR  | os.O_CREATE; \
               default:    __flag = os.O_RDONLY; \
               }} \
               __f, __err := os.OpenFile({path}, __flag, 0644); \
               if __err != nil {{ return -1 }} \
               return int32(__f.Fd()); \
             }}()"
        ),

        Backend::Unknown(kw) => return Err(format!(
            "'builtin::open' is not available for unknown backend '{kw}'"
        )),
    })
}
