# Bullang

A structured functional language that transpiles to Rust, Python, C, C++, and Go. Built to secure a good project architecture.

---

## Prerequisite

Having Cargo v1.92.0 installed

## Installation

### To install

```bash
cargo install --git https://github.com/My-sidequests/Bullang.git
```

### To update

```bash
bullang update                # pull latest stable
bullang update --experimental # experimental branch
```

---

## Overview of Bullang

### Function syntax

```
let add(a: i32, b: i32) -> result: i32 {
    (a, b) : a + b -> {result};
}
```

Each line within a function is a pipe: inputs on the left, expression in the middle, 
named binding on the right. The last binding is the return value.

### Some exemples of Bullang parameters

```bash
bullang init my_project --depth 3 --lang rs
bullang check
bullang convert my_project
bullang --help
```

If you need any help understanding a command, add the --help flag.
For exemple, bullang init --help will give you more details about init.

---

### What's inside a project

Every folder holds an `inventory.bu` — its rank declaration, optional language and library directives, struct definitions, and the list of source files with their functions.

Folders nest from `war` down to `skirmish`. Functions and structs defined at a lower rank are available one rank above. We build below to use above.

When initializing a project, a more in depth README will be created at the root of your folder.
This README will be automatically deleted on project conversion.

---

## Editor support

```bash
bullang lsp           # LSP server (stdin/stdout)
bullang editor-setup  # write config for Neovim, Helix, Emacs
```

VS Code: install the extension through VS Code extension page.
