# Bullang

A structured functional language that transpiles to Rust, Python, C, C++, and Go. Built to secure a good project architecture.

---

## Prerequisite

Having Cargo v1.92.0 installed

## Installation

```bash
cargo install --git https://github.com/My-sidequests/Bullang.git
```

## Updating

```bash
bullang update                   # stable branch
bullang update --experimental    # experimental branch (revert with bullang update)
```
---

## Core concepts

### The hierarchy

```
war → theater → battle → strategy → tactic → skirmish
```

Every folder has a rank, declared in `inventory.bu`. 
Skirmish is the leaf rank — source files live here, no sub-folders allowed. 
War is the root rank — sub-folders only, one main allowed, no source files.

**Flow-downward rule:** functions and structs defined at a lower rank are
available to the rank above. In order to use something, you must have it in a subfolder.
**Limits:** 5 sub-folders, 5 source files, 5 functions per file, 5 bullets per function.

### Source files

Bullang function declarations :

```
let add(a: i32, b: i32) -> result: i32 {
    (a, b) : a + b -> {result};
}
```

### Inventory files

Mandatory manifest of every folder.
It contains the rank, possible language and libraries.
This is where the structures must be declared, as well as the files
and the functions they contain.

```
#rank: tactic;
#lang: c;           ← optional: default target language for bullang convert
#lib: stdio.h;      ← optional: C/C++ header include, repeatable

struct Vec2 {
    x : f32,
    y : f32,
}

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
bullang init my_project --blueprint blueprint.bu
```

`--depth` sets the rank chain depth (1 = skirmish only, 6 = full war chain).
`--lang` writes `#lang:` to the root inventory.
`--blueprint` scaffolds from a blueprint file (see below).

### `bullang convert`

Transpile a project or a single `.bu` file:

```bash
# Project (reads #lang from inventory; defaults to Rust)
bullang convert my_project
bullang convert my_project -e py
bullang convert my_project -e c
bullang convert my_project -e cpp
bullang convert my_project -e go
bullang convert my_project -n my_lib          # custom output name
bullang convert my_project --out ~/projects/out
```

# Multi-language project (no -e — each folder converts to its declared language)

```bash
bullang convert my_project
```

# Single file (to stdout or -o)

```bash
bullang convert path/to/file.bu
bullang convert path/to/file.bu -e py
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
Useful to show possible errors before project conversion.

### `bullang fmt`

```bash
bullang fmt             # rewrite all .bu files to canonical style
bullang fmt --dry-run   # show what would change without writing
```

### `bullang stdlib --list`

List all available builtin functions.

---

## Language reference

### Types

| Bullang         | Rust           | Python        | C                | C++                       | Go               |
|-----------------|----------------|---------------|------------------|---------------------------|------------------|
| `i32`, `i64`    | same           | `int`         | `int32_t`        | `int32_t`                 | `int32`, `int64` |
| `f32`, `f64`    | same           | `float`       | `float`, `double`| same                      | `float32`, `float64` |
| `bool`          | same           | `bool`        | `bool`           | `bool`                    | `bool`           |
| `String`        | same           | `str`         | `char*`          | `std::string`             | `string`         |
| `Vec[T]`        | `Vec<T>`       | `List[T]`     | `vec_t` ¹        | `std::vector<T>`          | `[]T`            |
| `HashMap[K, V]` | `HashMap<K,V>` | `dict`        | `map_t` ¹        | `std::unordered_map<K,V>` | `map[K]V`        |
| `Option[T]`     | `Option<T>`    | `Optional[T]` | `T*`             | `std::optional<T>`        | `*T`             |
| `Result[T, E]`  | `Result<T,E>`  | —             | —                | —                         | —                |
| `Tuple[T, U]`   | `(T, U)`       | `Tuple[T,U]`  | N/A              | `std::tuple<T,U>`         | `Tuple_T_U` ²    |
| `Fn[T -> U]`    | `fn(T) -> U`   | `Callable`    | `void*`          | `std::function<U(T)>`     | `func(T) U`      |
| `&T`, `&mut T`  | same           | —             | `T*`             | `const T&`, `T&`          | `*T`             |

¹ C projects using `Vec[T]` or `HashMap[K,V]` get `foreign_types.h` emitted
automatically. `map_t` uses string keys only.

² Go has no built-in tuple — Bullang emits a named struct per unique combination
into `types.go` (e.g. `Tuple[i32, f64]` → `Tuple_i32_f64`).

### Operators

`+`  `-`  `*`  `/`  `%`  `==`  `!=`  `<`  `>`  `<=`  `>=`

Comparison operators return `bool`. `String + String` is string concatenation.

### String interpolation

`{identifier}` placeholders in string literals are interpolated natively:

```
() : "value is {x}" -> {msg};
```

| Backend | Output                                         |
|---------|------------------------------------------------|
| Rust    | `format!("value is {x}")`                      |
| Python  | `f"value is {x}"`                              |
| Go      | `fmt.Sprintf("value is %v", x)` (auto-import)  |
| C/C++   | `snprintf(buf, sizeof(buf), "value is %s", x)` |

### Error propagation (`?`)

Add `?` after a binding to propagate `None` or `Err` early. Only valid when
the function's output type is `Option[T]` or `Result[T, E]`. Cannot appear
on the last bullet.

```
let safe_div(a: f64, b: f64) -> result: Option[f64] {
    (b) : b != 0.0 -> {valid}?;
    (a, b) : a / b -> {result};
}
```

The binding after `?` holds the unwrapped inner type. Per-backend emission:

| Backend | Check                              |
|---------|------------------------------------|
| Rust    | `expr?`                            |
| Python  | `if x is None: return None`        |
| Go      | `if x == nil { return nil }`       |
| C       | `if (!x) { return NULL; }`         |
| C++     | `if (!x) { return std::nullopt; }` |


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

## Blueprint files

Describe the full project structure in a single file. Use a language prefix
to set the target language for a folder and all its descendants:

```
war my_project {
    rust: engine {
        battle_engine {}
    }
    python: pipeline {
        battle_pipeline {}
    }
}
```

```bash
bullang init my_project --blueprint blueprint.bu
```

The blueprint stays in the root after `bullang convert` and acts as the
project's language topology manifest.

---

## Multi-language projects

```bash
bullang convert my_project        # each folder → its declared language
bullang convert my_project -e rs  # ERROR: project uses multiple languages
```

Each language subtree outputs to `_foldername` next to the source folder.

---

## Editor support

```bash
bullang lsp           # start the LSP server (stdin/stdout)
bullang editor-setup  # write config for Neovim, Helix, Emacs
```

VS Code: install the `.vsix` from the releases page.
