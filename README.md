# Bullang 1.0.0

A structured functional language that transpiles to Rust, Python, C, C++, and Go.

---

## Installation

```bash
git clone https://github.com/My-sidequests/Bullang.git
cd Bullang
cargo build --release
sudo ./target/release/bullang install
```

## Updating

```bash
bullang update
```

On first run, clones the repository to `~/.local/share/bullang/src` and builds.
On subsequent runs, `git pull`s the latest changes and rebuilds. Requires `git`
and `cargo` on PATH. Run with `sudo` if the binary is in a system directory.

---

## Core concepts

### The hierarchy

```
war → theater → battle → strategy → tactic → skirmish
```

Every folder has exactly one rank declared in `inventory.bu`. Skirmish is the
leaf rank — source files live here, no sub-folders allowed. War is the root
rank — sub-folders only, no source files.

**Limits:** 5 sub-folders, 5 source files, 5 functions per file, 5 bullets per function.

### Source files

Pure function declarations — no imports, no exports:

```
let add(a: i32, b: i32) -> result: i32 {
    (a, b) : a + b -> {result};
}
```

### Inventory files

Mandatory manifest of every folder:

```
#rank: tactic;
#lang: c;           ← optional: default target language for bullang convert
#lib: stdio.h;      ← optional: C/C++ header include, repeatable

math : add, subtract, multiply;
ops  : clamp, abs_val;
```

---

## Commands

### `bullang init`

```bash
bullang init my_project --depth 2
bullang init my_c_project --depth 4 --lang c --lib stdio.h --lib math.h
bullang init my_go_service --depth 3 --lang go
```

`--depth` sets the rank chain depth (1 = skirmish only, 6 = full war chain).
`--lang` writes `#lang:` to the root inventory so `bullang convert` knows the
target language without `-e`.

### `bullang convert`

Transpile a project **or** a single `.bu` file:

```bash
# Project (reads #lang from inventory; defaults to Rust)
bullang convert my_project
bullang convert my_project -e py
bullang convert my_project -e c
bullang convert my_project -e cpp
bullang convert my_project -e go
bullang convert my_project -n my_lib          # custom output name
bullang convert my_project --out ~/projects/out

# Single file (to stdout or -o)
bullang convert path/to/file.bu
bullang convert path/to/file.bu -e py
bullang convert path/to/file.bu -o out.rs
```

**To run the output:**

```bash
cd my_lib && cargo build && cargo run     # Rust
cd my_lib && python3 -m my_lib            # Python
cd my_lib && make && ./my_lib             # C / C++
cd my_lib && go run .                     # Go
```

### `bullang check`

Validate and type-check from anywhere in the tree. Finds the root automatically.
All errors are shown in one run, grouped by file.

### `bullang update`

```bash
bullang update               # update from the official repo
sudo bullang update          # if installed to a system directory
```

### `bullang stdlib --list`

List all available builtin functions.

### `bullang install`

Install to system PATH.

---

## Language reference

### Types

| Bullang | Rust | Python | C | C++ | Go |
|---------|------|--------|---|-----|----|
| `i32`, `i64` | same | `int` | `int32_t` | `int32_t` | `int32`, `int64` |
| `f32`, `f64` | same | `float` | `float`, `double` | same | `float32`, `float64` |
| `bool` | same | `bool` | `bool` | `bool` | `bool` |
| `String` | same | `str` | `char*` | `std::string` | `string` |
| `Vec[T]` | `Vec<T>` | `List[T]` | `T*` | `std::vector<T>` | `[]T` |
| `Option[T]` | `Option<T>` | `Optional[T]` | `T*` | `std::optional<T>` | `*T` |
| `Tuple[T, U]` | `(T, U)` | `Tuple[T,U]` | struct | `std::tuple<T,U>` | struct |
| `Fn[T -> U]` | `fn(T) -> U` | `Callable[[T],U]` | `void*` | `std::function<U(T)>` | `func(T) U` |
| `&T`, `&mut T` | same | — | `T*` | `const T&`, `T&` | `*T` |
| `()` | same | `None` | `void` | `void` | (omitted) |

### Bullet (pipe) syntax

```
(input1, input2) : expression -> {binding_name};
```

Rules: every binding must be consumed by a later bullet (except the final output);
no binding may be assigned twice; the last bullet must bind to the declared output name.

### Native escape blocks

```
let sum_vec(values: Vec[i32]) -> result: i32 {
    @rust
    values.iter().sum()
    @end
}
```

Backends: `@rust`  `@python`  `@c`  `@cpp`  `@go`

`@c` is also valid in `@cpp` builds. All other cross-backend combinations are errors.

### `builtin::name` — standard library

```
let upper(s: String) -> result: String {
    builtin::to_upper
}
```

The function's declared parameters are passed to the builtin in order.
All 13 builtins work in every backend (Rust, Python, C, C++, Go):

**Math:** `abs`  `pow`  `powf`  `sqrt`  `clamp`

**String:** `to_upper`  `to_lower`  `trim`  `starts_with`  `ends_with`
`replace_str`  `to_string`  `parse_i64`

Run `bullang stdlib --list` for full signatures.

### `main.bu`

Entry point. Never listed in inventory. Allowed at any rank except skirmish.

```
let main() -> result: () {
    @rust
    println!("Hello from Bullang!");
    @end
}
```

---

## Error messages

**Inventory / folder:**
- `Missing inventory.bu in '<dir>'`
- `Source file '<n>.bu' exists but is not listed in inventory`
- `Function '<fn>' exists in '<file>.bu' but is not listed in inventory`
- `<Rank> folder cannot contain more than 5 source files`

**Function / bullet:**
- `Function '<n>': cannot contain more than 5 bullets`
- `Function '<n>' bullet N: '<x>' is an unknown parameter`
- `Function '<n>': '{<x>}' is produced but never used`
- `'builtin::<n>' is not a known builtin`
- `'@<kw>' block cannot be used when building for '<backend>' backend`

**Type:**
- `Function '<n>': last bullet produces <A> but declared output is <B>`
- `Function '<n>': operator '<op>' requires both sides to be the same type`

---

## Roadmap

| Feature | Status |
|---------|--------|
| Rust backend | ✓ |
| Python backend | ✓ |
| C backend | ✓ |
| C++ backend | ✓ |
| Go backend | ✓ |
| `builtin::` stdlib — 13 universal builtins | ✓ |
| `bullang init --lang` / `#lang:` directive | ✓ |
| `bullang update` | ✓ |
| `bullang convert` (project + single file, unified) | ✓ |
| Error recovery — all errors in one run | ✓ |
| New type syntax `Vec[T]` `Tuple[T,U]` `Fn[T->U]` | ✓ |
| Language spec (SPEC.md) | ✓ |
| Language server (editor integration) | Next |
