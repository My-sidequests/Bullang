# Bullang

A structured functional language that transpiles to Rust, Python, C, C++ and Go.

Bullang enforces a strict hierarchy of folders, a hard limit on complexity at every
level, and zero metadata inside source files. The result is code that is always easy
to navigate, always honest about what it does, and always translatable to any target
language.

---

## Installation

```bash
git clone https://github.com/My-sidequests/Bullang.git bullang
cd bullang
cargo build --release
sudo ./target/release/bullang install
bullang --version
```

---

## Commands

### `bullang init`

Scaffold a new project.

```bash
bullang init my_project --depth 2
bullang init my_c_project --depth 4 --lib stdio.h --lib math.h
```

| Depth | Root rank |
|-------|-----------|
| 1 | skirmish |
| 2 | tactic → skirmish |
| 3 | strategy → tactic → skirmish |
| 4 | battle → strategy → tactic → skirmish |
| 5 | theater → … → skirmish |
| 6 | war → … → skirmish |

`--lib` is repeatable. Each library becomes a `#lib:` entry in the root inventory
and a `#include` in the generated C/C++ header.

### `bullang check`

Validate and type-check from anywhere inside the tree. Bullang finds the root
automatically by walking up, like `tsc` with `tsconfig.json`.

### `bullang convert`

Transpile the project.

```bash
bullang convert my_project              # → _my_project/, Rust
bullang convert my_project -n my_lib    # custom output name
bullang convert my_project -e py        # Python
bullang convert my_project -e c         # C
bullang convert my_project -e cpp       # C++
bullang convert my_project --out ~/lib  # explicit path
```

To run the output:

```bash
# Rust
cd my_lib && cargo build && cargo run

# Python
cd my_lib && python3 -m my_lib

# C / C++
cd my_lib && make && ./my_lib
```

### `bullang stdlib --list`

List all standard library builtins.

### `bullang file`

Transpile a single `.bu` file to stdout.

### `bullang install`

Install to system PATH.

---

## Language reference

### Core concepts

Every Bullang project is a folder tree. Each folder has exactly one rank
(war → theater → battle → strategy → tactic → skirmish). Every folder has
an `inventory.bu` that lists all its source files and functions.

Source files contain only function declarations — no imports, no metadata:

```
let add(a: i32, b: i32) -> result: i32 {
    (a, b) : a + b -> {result};
}
```

A **bullet** is one `(inputs) : expression -> {binding};` statement.
Functions may have at most 5 bullets.

### Inventory format

```
#rank: tactic;
#lib: stdio.h;      ← optional, C/C++ only, repeatable

math    : add, subtract, multiply;
helpers : clamp, abs_val;
```

### Native escape blocks

```
let sum_vec(values: Vec<i32>) -> result: i32 {
    @rust
    values.iter().sum()
    @end
}
```

Available backends: `@rust`, `@python`, `@c`, `@cpp`.
`@c` is also valid inside `@cpp` builds. All other cross-backend uses are errors.

### `@builtin` stdlib

```
let my_sum(values: Vec<i32>) -> result: i32 {
    @builtin sum
}
```

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

## Type mapping

| Bullang | Rust | Python | C | C++ |
|---------|------|--------|---|-----|
| `i8`–`i64` | same | `int` | `int8_t`–`int64_t` | `int8_t`–`int64_t` |
| `u8`–`u64` | same | `int` | `uint8_t`–`uint64_t` | `uint8_t`–`uint64_t` |
| `usize` | same | `int` | `size_t` | `size_t` |
| `f32`, `f64` | same | `float` | `float`, `double` | `float`, `double` |
| `bool` | same | `bool` | `bool` | `bool` |
| `String` | same | `str` | `char*` | `std::string` |
| `&str` | same | `str` | `char*` | `std::string_view` |
| `&T` | same | — | `T*` | `const T&` |
| `&mut T` | same | — | `T*` | `T&` |
| `Vec<T>` | same | `List[T]` | `T*` | `std::vector<T>` |
| `(T, U)` | same | `Tuple[T, U]` | struct | `std::tuple<T, U>` |
| `Option<T>` | same | `Optional[T]` | `T*` | `std::optional<T>` |
| `fn(T)->U` | same | `Callable[[T],U]` | `void*` | `std::function<U(T)>` |
| `()` | same | `None` | `void` | `void` |

---

## Standard library

Run `bullang stdlib --list` for the full list.

### String operations
`trim`, `to_upper`, `to_lower`, `starts_with`, 
`ends_with`, `replace_str`, `parse_i64`, `to_string`

### Math
`abs`, `pow`, `powf`, `sqrt`, `clamp`

**Backend support:** All builtins work in Rust and Python. C supports scalar
math and string-length builtins. C++ supports the full set via STL.

---

## Error messages

### Inventory / folder
- `Missing inventory.bu in '<dir>'`
- `War folder cannot contain source files`
- `War folder cannot exceed 5 theaters`
- `Skirmish folder cannot contain sub-folders`
- `<Rank> folder cannot contain more than 5 source files`
- `<Rank> folder cannot contain more than 5 <child> sub-folders`
- `Source file '<n>.bu' exists but is not listed in inventory`
- `Inventory lists '<n>' but '<n>.bu' does not exist`
- `Function '<fn>' exists in '<file>.bu' but is not listed in inventory`
- `The function '<fn>' is listed in inventory, but not found in '<file>.bu'`

### Function / bullet
- `A source file cannot contain more than 5 functions`
- `Function '<n>': cannot contain more than 5 bullets`
- `Function '<n>' bullet N: '<x>' is an unknown parameter`
- `Function '<n>': '{<x>}' is assigned more than once`
- `Function '<n>': last bullet output '{<x>}' must match function output '{<y>}'`
- `Function '<n>': '{<x>}' is produced but never used`
- `Function '<n>': skirmish files cannot call other functions`
- `Function '<n>': calls '<fn>' which is not listed in any child inventory`
- `'@builtin <n>' is not a known builtin`
- `'@<kw>' is not a supported backend`
- `'@rust' block cannot be used when building for 'python' backend`

### Type
- `Function '<n>': last bullet produces <A> but declared output is <B>`
- `Function '<n>': operator '<op>' requires both sides to be the same type`
- `Function '<n>': operator '<op>' requires a numeric type, got <T>`
- `Function '<n>': '<fn>' expects N argument(s) but received M`
- `Function '<n>': argument N passed to '<fn>' is <A> but <B> was expected`
