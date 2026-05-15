use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "out",
    "(fd: i32, content: String) → i32",
    "Write a string to a file descriptor. Returns bytes written, -1 on error",
);

// File name is fd_out.rs to stay consistent with fd_in.rs naming convention.
// The builtin name in Bullang source is `builtin::out`.

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("out", params, 2)?;
    let (fd, content) = (p[0], p[1]);

    Ok(match backend {
        // ── Rust ─────────────────────────────────────────────────────────────
        // Wraps the raw fd in a ManuallyDrop<File> so we can write without
        // the File destructor closing the fd on drop.
        Backend::Rust => format!(
            "{{\
               use std::io::Write;\
               use std::os::unix::io::FromRawFd;\
               use std::mem::ManuallyDrop;\
               let mut __f = ManuallyDrop::new(unsafe {{ \
                 std::fs::File::from_raw_fd({fd}) \
               }});\
               let __bytes = {content}.as_bytes();\
               __f.write_all(__bytes).map(|_| __bytes.len() as i32).unwrap_or(-1)\
             }}"
        ),

        // ── Python ───────────────────────────────────────────────────────────
        // os.write returns bytes written directly.
        Backend::Python => {
            let fd = super::py_esc(fd);
            let content = super::py_esc(content);
            format!(
                "(lambda __os, __fd, __s: \
                   (lambda __b: __os.write(__fd, __b))({content}.encode('utf-8'))\
                 )(__import__('os'), {fd}, {content})"
            )
        }

        // ── C ────────────────────────────────────────────────────────────────
        // write(2); returns ssize_t cast to int32_t.
        Backend::C => format!(
            "(int32_t)write({fd}, {content}, strlen({content}))"
        ),

        // ── C++ ──────────────────────────────────────────────────────────────
        Backend::Cpp => format!(
            "static_cast<int32_t>(write({fd}, {content}.c_str(), {content}.size()))"
        ),

        // ── Go ───────────────────────────────────────────────────────────────
        // syscall.Write returns (n int, err error).
        Backend::Go => format!(
            "func() int32 {{ \
               __b := []byte({content}); \
               __n, __err := syscall.Write(uintptr({fd}), __b); \
               if __err != nil {{ return -1 }} \
               return int32(__n); \
             }}()"
        ),

        Backend::Unknown(kw) => return Err(format!(
            "'builtin::out' is not available for unknown backend '{kw}'"
        )),
    })
}
