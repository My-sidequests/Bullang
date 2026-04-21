# Bullang for VS Code

Language support for [Bullang](https://github.com/My-sidequests/Bullang) (`.bu` files).

## Features

- **Syntax highlighting** — keywords, types, directives, native blocks, builtins
- **Diagnostics** — parse, validation, and type errors shown inline
- **Hover** — function signature on hover
- **Go-to-definition** — jump to a function's declaration

## Requirements

Cargo version 1.92.0.

`bullang` must be installed on your system. To install it:

```bash
git clone https://github.com/My-sidequests/Bullang.git Bullang
cd Bullang && cargo build --release
./target/release/bullang install
```

If already installed, consider: `bullang update`

## Connecting the Language Server

### Find your absolute path

- Linux / macOS: Run which bullang in your terminal.
- Windows: Run where bullang in PowerShell.

### Update VSCode path for Bullang

- Copy the path (looks like /usr/local/bin/bullang or C:\Users\You\Bullang\bullang.exe).
- Open VS Code settings and search for "Bullang Server Path".
- In the setting box, replace "bullang" with the path.
- Reload VS Code
- Enjoy !
