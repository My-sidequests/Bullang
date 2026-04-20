# Bullang for VS Code

Language support for [Bullang](https://github.com/My-sidequests/Bullang) (`.bu` files).

## Features

- **Syntax highlighting** — keywords, types, directives, native blocks, builtins
- **Diagnostics** — parse, validation, and type errors shown inline
- **Hover** — function signature on hover
- **Go-to-definition** — jump to a function's declaration

## Requirements

`bullang` must be installed and on your PATH:

```bash
git clone https://github.com/My-sidequests/Bullang.git
cd Bullang && cargo build --release
sudo ./target/release/bullang install
```

Or after your first install: `bullang update`

## Installing

### From a .vsix file

```bash
cd bullang-vscode
npm install
npm run compile
npx vsce package          # produces bullang-1.0.0.vsix
code --install-extension bullang-1.0.0.vsix
```

### Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `bullang.serverPath` | `"bullang"` | Path to bullang if not on PATH |
| `bullang.trace.server` | `"off"` | LSP tracing: `off`, `messages`, `verbose` |
