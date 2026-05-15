use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "close",
    "(fd: i32)                 → i32",
    "Close a file descriptor. Returns 0 on success, -1 on error",
);

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("close", params, 1)?;
    let fd = p[0];

    Ok(match backend {
        // ── Rust ─────────────────────────────────────────────────────────────
        // Construct a File from the raw fd and let it drop — the Drop impl
        // calls close(2) internally.  Returns 0 unconditionally since
        // std::fs::File::drop does not surface the error; this matches
        // the common usage pattern of close in C.
        Backend::Rust => format!(
            "{{\
               unsafe {{ drop(std::fs::File::from_raw_fd({fd})) }};\
               0i32\
             }}"
        ),

        // ── Python ───────────────────────────────────────────────────────────
        // os.close returns None; normalise to 0 / -1.
        Backend::Python => {
            let fd = super::py_esc(fd);
            format!(
                "(lambda __os, __fd: \
                   (lambda __r: 0)(__os.close(__fd)) \
                   if True else -1\
                 )(__import__('os'), {fd})"
            )
        }

        // ── C ────────────────────────────────────────────────────────────────
        // POSIX close(2) directly. Requires <unistd.h>.
        Backend::C => format!("close({fd})"),

        // ── C++ ──────────────────────────────────────────────────────────────
        Backend::Cpp => format!("close({fd})"),

        // ── Go ───────────────────────────────────────────────────────────────
        // syscall.Close returns an error; normalise to 0 / -1.
        Backend::Go => format!(
            "func() int32 {{\
               if __err := syscall.Close(uintptr({fd})); __err != nil {{ return -1 }}\
               return 0\
             }}()"
        ),

        Backend::Unknown(kw) => return Err(format!(
            "'builtin::close' is not available for unknown backend '{kw}'"
        )),
    })
}
