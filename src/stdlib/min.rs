use crate::ast::{Backend, Param};

pub const META: (&str, &str, &str) = (
    "min",
    "(arr: Vec[T])             → T",
    "Minimum value in an array — linear scan, no delegation to language primitives",
);

// Empty array behaviour (consistent across all backends):
//   - Rust:   panic with a clear message
//   - Python: raises ValueError
//   - C:      returns INT64_MIN (sentinel — caller must check)
//   - C++:    returns INT64_MIN
//   - Go:     returns math.MinInt64

pub fn emit(params: &[Param], backend: &Backend) -> Result<String, String> {
    let p = super::need("min", params, 1)?;
    let arr = p[0];

    Ok(match backend {
        // ── Rust ─────────────────────────────────────────────────────────────
        // Block expression with a local fn so the Ord bound is expressed once.
        Backend::Rust => format!(
            "{{\
               fn __min<T: Ord + Clone>(v: &[T]) -> T {{\
                 if v.is_empty() {{ panic!(\"builtin::min called on empty Vec\"); }}\
                 let mut __m = v[0].clone();\
                 let mut __i = 1usize;\
                 while __i < v.len() {{\
                   if v[__i] < __m {{ __m = v[__i].clone(); }}\
                   __i += 1;\
                 }}\
                 __m\
               }}\
               __min(&{arr})\
             }}"
        ),

        // ── Python ───────────────────────────────────────────────────────────
        // Recursive lambda — linear scan, no call to min().
        Backend::Python => {
            let arr = super::py_esc(arr);
            format!(
                "(lambda __f: __f(__f, list({arr})))(\
                   lambda __f, __a: \
                     (lambda: (_ for _ in ()).throw(ValueError('builtin::min called on empty list')))() \
                     if not __a else \
                     (lambda __m: \
                       (lambda __g: __g(__g, __a[1:], __m))(\
                         lambda __g, __r, __m: __m if not __r \
                           else __g(__g, __r[1:], __r[0] if __r[0] < __m else __m)\
                       )\
                     )(__a[0])\
                 )"
            )
        }

        // ── C ────────────────────────────────────────────────────────────────
        // GCC compound statement over the vec_t*; returns INT64_MIN on empty.
        Backend::C => format!(
            "({{ \
               vec_t *__src = {arr}; \
               size_t __n = vec_len(__src); \
               int64_t __m = INT64_MIN; \
               if (__n > 0) {{ \
                 __m = *(int64_t *)vec_get(__src, 0); \
                 for (size_t __i = 1; __i < __n; __i++) {{ \
                   int64_t __v = *(int64_t *)vec_get(__src, __i); \
                   if (__v < __m) __m = __v; \
                 }} \
               }} \
               __m; \
             }})"
        ),

        // ── C++ ──────────────────────────────────────────────────────────────
        Backend::Cpp => format!(
            "[&]() -> int64_t {{ \
               const auto &__v = {arr}; \
               if (__v.empty()) return INT64_MIN; \
               int64_t __m = static_cast<int64_t>(__v[0]); \
               for (size_t __i = 1; __i < __v.size(); ++__i) \
                 if (static_cast<int64_t>(__v[__i]) < __m) \
                   __m = static_cast<int64_t>(__v[__i]); \
               return __m; \
             }}()"
        ),

        // ── Go ───────────────────────────────────────────────────────────────
        Backend::Go => format!(
            "func() int64 {{ \
               __v := {arr}; \
               if len(__v) == 0 {{ return math.MinInt64 }} \
               __m := __v[0]; \
               for __i := 1; __i < len(__v); __i++ {{ \
                 if __v[__i] < __m {{ __m = __v[__i] }} \
               }} \
               return __m; \
             }}()"
        ),

        Backend::Unknown(kw) => return Err(format!(
            "'builtin::min' is not available for unknown backend '{kw}'"
        )),
    })
}
