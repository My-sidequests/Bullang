use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "in",
    "(fd: i32)                 → String",
    "Read one line from a file descriptor (newline stripped). Empty string on EOF/error",
);

// File name is fd_in.rs because `in` is a Rust reserved keyword and cannot
// be used as a module name.  The builtin name in Bullang source remains
// `builtin::in`.

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("in", params, 1)?;
    let fd = p[0];

    Ok(match backend {
        // ── Rust ─────────────────────────────────────────────────────────────
        // Wraps the raw fd in a BufReader; reads one line; strips the newline.
        // The fd is NOT closed here — ownership stays with the caller.
        Backend::Rust => format!(
            "{{\
               use std::io::{{BufRead, BufReader}};\
               use std::os::unix::io::FromRawFd;\
               let __fd = {fd};\
               let __f  = unsafe {{ std::fs::File::from_raw_fd(__fd) }};\
               let mut __r   = BufReader::new(&__f);\
               let mut __line = String::new();\
               let _ = __r.read_line(&mut __line);\
               std::mem::forget(__f);\
               __line.trim_end_matches('\\n').trim_end_matches('\\r').to_owned()\
             }}"
        ),

        // ── Python ───────────────────────────────────────────────────────────
        // os.read reads raw bytes; decode to str; strip trailing newline.
        Backend::Python => {
            let fd = super::py_esc(fd);
            format!(
                "(lambda __os, __fd: \
                   (lambda __chunks: \
                     b''.join(__chunks).decode('utf-8', errors='replace').rstrip('\\n').rstrip('\\r')\
                   )(\
                     (lambda __f: __f(__f, __os, __fd, []))(\
                       lambda __f, __os, __fd, __acc: __acc \
                         if (__acc and __acc[-1][-1:] == b'\\n') or not __acc and False \
                         else \
                           (lambda __b: \
                             __acc if not __b \
                             else __f(__f, __os, __fd, __acc + [__b]) \
                               if __b != b'\\n' \
                             else __acc + [__b] \
                           )(__os.read(__fd, 1))\
                     )\
                   )\
                 )(__import__('os'), {fd})"
            )
        }

        // ── C ────────────────────────────────────────────────────────────────
        // read(2) byte-by-byte into a local buffer until '\\n' or EOF.
        // Returns a heap-allocated char* (caller owns it).
        Backend::C => format!(
            "({{ \
               char *__buf = (char *)malloc(4096); \
               size_t __i = 0; \
               char __ch; \
               ssize_t __n; \
               while (__i < 4095 && (__n = read({fd}, &__ch, 1)) > 0) {{ \
                 if (__ch == '\\n') break; \
                 __buf[__i++] = __ch; \
               }} \
               if (__i > 0 && __buf[__i-1] == '\\r') __i--; \
               __buf[__i] = '\\0'; \
               __buf; \
             }})"
        ),

        // ── C++ ──────────────────────────────────────────────────────────────
        // IIFE wrapping the same byte-by-byte read; returns std::string.
        Backend::Cpp => format!(
            "[&]() -> std::string {{ \
               std::string __s; \
               char __ch; \
               while (read({fd}, &__ch, 1) > 0) {{ \
                 if (__ch == '\\n') break; \
                 __s += __ch; \
               }} \
               if (!__s.empty() && __s.back() == '\\r') __s.pop_back(); \
               return __s; \
             }}()"
        ),

        // ── Go ───────────────────────────────────────────────────────────────
        // Wraps the raw fd in an os.File (without taking ownership via
        // runtime.SetFinalizer) then reads one line through a bufio.Reader.
        Backend::Go => format!(
            "func() string {{ \
               __f := os.NewFile(uintptr({fd}), \"\"); \
               if __f == nil {{ return \"\" }} \
               __r := bufio.NewReader(__f); \
               __line, _ := __r.ReadString('\\n'); \
               __line = strings.TrimRight(__line, \"\\r\\n\"); \
               runtime.KeepAlive(__f); \
               return __line; \
             }}()"
        ),

        Backend::Unknown(kw) => return Err(format!(
            "'builtin::in' is not available for unknown backend '{kw}'"
        )),
    })
}
