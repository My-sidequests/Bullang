# Bullang

A structured functional language that transpiles to Rust, Python, C, C++, and Go.

Bullang enforces a strict hierarchy of folders, a hard limit on complexity at
every level, and zero metadata inside source files. Code is always easy to
navigate, always honest about what it does, and always translatable to any
target language.

---

## Installation

```bash
git clone https://github.com/My-sidequests/Bullang.git
cd Bullang
cargo build --release
sudo ./target/release/bullang install
bullang --version
```

## Updating

```bash
bullang update
```

Fetches the latest source from the repository, builds a release binary, and
reinstalls it in-place. Requires `git` and `cargo` on your PATH.

---

## Core concepts

### The hierarchy

Every Bullang project is a folder tree. Each folder has exactly one rank:

```
war
└── theater
    └── battle
        └── strategy
            └── tactic
                └── skirmish     ← leaf: where source files live
```

**Rules:**
- Every folder must have an `inventory.bu` declaring its rank.
- War may only contain theater sub-folders — no source files.
- Skirmish may only contain source files — no sub-folders.
- Middle ranks may have up to **5 sub-folders** and **5 source files**.
- Maximum **5 functions** per source file.
- Maximum **5 bullets** per function.

### Source files

Pure function declarations — no imports, no exports, no metadata:

```
let add(a: i32, b: i32) -> result: i32 {
    (a, b) : a + b -> {result};
}
```

### Inventory files

The mandatory manifest of every folder:

```
#rank: tactic;
#lang: c;           ← optional: declared target language
#lib: stdio.h;      ← optional: C/C++ header, repeatable

math    : add, subtract, multiply;
helpers : clamp, abs_val;
```

---

## Commands

### `bullang init`

Scaffold a new project:

```bash
bullang init my_project --depth 2
bullang init my_c_project --depth 4 --lang c --lib stdio.h --lib math.h
bullang init my_go_service --depth 3 --lang go
```

`--lang <ext>` writes `#lang: ext;` to the root inventory so `bullang convert`
knows the target language without needing `-e`.

`--lang` values: `rs` `py` `c` `cpp` `go`

| Depth | Root rank |
|-------|-----------|
| 1 | skirmish |
| 2 | tactic → skirmish |
| 3 | strategy → tactic → skirmish |
| 4 | battle → strategy → tactic → skirmish |
| 5 | theater → … → skirmish |
| 6 | war → … → skirmish |

### `bullang convert`

Transpile the project:

```bash
# Uses #lang from inventory if present, otherwise defaults to Rust
bullang convert my_project

# Explicit target (overrides #lang)
bullang convert my_project -e py
bullang convert my_project -e c
bullang convert my_project -e cpp
bullang convert my_project -e go

# Custom output name or path
bullang convert my_project -n my_lib
bullang convert my_project --out ~/projects/my_lib
```

**To run the output:**

```bash
# Rust
cd my_lib && cargo build && cargo run

# Python
cd my_lib && python3 -m my_lib

# C / C++
cd my_lib && make && ./my_lib

# Go
cd my_lib && go run .
```

### `bullang check`

Validate and type-check from anywhere inside the tree. Bullang finds the root
automatically, like `tsc` with `tsconfig.json`.

### `bullang update`

Update to the latest version from the repository:

```bash
bullang update
# or with an explicit repo URL:
bullang update --repo https://github.com/My-sidequests/Bullang.git
```

### `bullang stdlib --list`

List all available builtin functions and their signatures.

### `bullang file`

Transpile a single `.bu` file to stdout.

### `bullang install`

Install to system PATH.

---

## Language reference

### Function syntax

```
let name(param1: Type, param2: Type) -> output_name: ReturnType {
    body
}
```

### Bullet (pipe) syntax

```
(input1, input2) : expression -> {binding_name};
```

### Types

| Bullang | Rust | Python | C | C++ | Go |
|---------|------|--------|---|-----|----|
| `i32`, `i64` | same | `int` | `int32_t`, `int64_t` | same | `int32`, `int64` |
| `f32`, `f64` | same | `float` | `float`, `double` | same | `float32`, `float64` |
| `bool` | same | `bool` | `bool` | `bool` | `bool` |
| `String` | same | `str` | `char*` | `std::string` | `string` |
| `Vec[T]` | `Vec<T>` | `List[T]` | `T*` | `std::vector<T>` | `[]T` |
| `Option[T]` | `Option<T>` | `Optional[T]` | `T*` | `std::optional<T>` | `*T` |
| `Tuple[T, U]` | `(T, U)` | `Tuple[T, U]` | struct | `std::tuple<T, U>` | `struct{V0 T; V1 U}` |
| `Fn[T -> U]` | `fn(T) -> U` | `Callable[[T], U]` | `void*` | `std::function<U(T)>` | `func(T) U` |
| `&T`, `&mut T` | same | — | `T*` | `const T&`, `T&` | `*T` |
| `()` | same | `None` | `void` | `void` | (omitted) |

### Native escape blocks

```
let sum_vec(values: Vec[i32]) -> result: i32 {
    @rust
    values.iter().sum()
    @end
}
```

Supported backends: `@rust` `@python` `@c` `@cpp` `@go`

`@c` is also valid in `@cpp` builds. All other cross-backend uses are errors.

### `builtin::name` — standard library

```
let upper(s: String) -> result: String {
    builtin::to_upper
}
```

The function's declared parameters are passed to the builtin in order.

**Universal builtins (all 5 backends):**

| Category | Builtins |
|----------|----------|
| Math | `abs` `pow` `powf` `sqrt` `clamp` |
| String | `to_upper` `to_lower` `trim` `starts_with` `ends_with` `replace_str` `to_string` `parse_i64` |

Run `bullang stdlib --list` for signatures.

### `main.bu`

Entry point. Never listed in `inventory.bu`. Allowed at any rank except skirmish.

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

## Example projects

### Minimal Rust library (depth 1)

```bash
bullang init my_math --depth 1 --lang rs
cd my_math
# edit example.bu
bullang check
bullang convert my_math -n my_math_out
cd my_math_out && cargo build
```

### C project with libraries (depth 3)

```bash
bullang init my_c --depth 3 --lang c --lib stdio.h --lib math.h
cd my_c
bullang check
bullang convert my_c -n my_c_out    # uses #lang: c automatically
cd my_c_out && make && ./my_c_out
```

### Go service (depth 2)

```bash
bullang init my_go --depth 2 --lang go
cd my_go
bullang check
bullang convert my_go -n my_go_out  # uses #lang: go automatically
cd my_go_out && go run .
```

---

## Roadmap

| Feature | Status |
|---------|--------|
| Rust backend | ✓ |
| Python backend | ✓ |
| C backend | ✓ |
| C++ backend | ✓ |
| Go backend | ✓ |
| `builtin::` stdlib (13 universal builtins) | ✓ |
| `bullang init --lang` / `#lang:` directive | ✓ |
| `bullang update` (auto-update from repo) | ✓ |
| Error recovery (all errors in one run) | ✓ |
| New type syntax (`Vec[T]`, `Tuple[T,U]`, `Fn[T->U]`) | ✓ |
| Language spec (SPEC.md) | ✓ |
| Language server (editor integration) | Planned |
