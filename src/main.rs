/// The canonical source repository. Override with `bullang update --repo <url>`.
/// Change this to your real repository URL before distributing.
const DEFAULT_REPO: &str = "https://github.com/My-sidequests/Bullang.git";

mod ast;
mod build;
mod lsp;
mod codegen;
mod codegen_c;
mod codegen_cpp;
mod codegen_go;
mod codegen_python;
mod init;
mod parser;
mod stdlib;
mod typecheck;
mod validator;

use clap::{Parser as ClapParser, Subcommand};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use crate::ast::Backend;
use crate::validator::AllErrors;

#[derive(ClapParser)]
#[command(
    name    = "bullang",
    version = env!("CARGO_PKG_VERSION"),
    about   = "Bullang (.bu) transpiler\n\n\
               Install once with `bullang install`, then run from anywhere.\n\
               The source tree is never modified — all output is external."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Install bullang to your system PATH.
    Install,

    /// Scaffold a new Bullang project.
    ///
    /// Examples:
    ///
    ///   bullang init my_project --depth 2
    ///
    ///   bullang init my_project --depth 4 --lang c --lib stdio.h
    ///
    ///   bullang init my_project --blueprint blueprint.bu
    ///
    ///   bullang init my_project --blueprint blueprint.bu --lang go
    Init {
        /// Name of the project folder to create
        name: String,
        /// Hierarchy depth: 1 = skirmish, 2 = tactic+skirmish, … 6 = full war chain
        /// (ignored when --blueprint is used — depth is inferred from the blueprint)
        #[arg(short, long, default_value = "2")]
        depth: u8,
        /// Path to a blueprint.bu file describing the project structure.
        /// The blueprint is copied to the project root unchanged.
        #[arg(long, value_name = "FILE")]
        blueprint: Option<PathBuf>,
        /// Target language (rs, py, c, cpp, go). Written to inventory as #lang:.
        #[arg(long, value_name = "EXT")]
        lang: Option<String>,
        /// External library to declare (repeatable). Used as #include <lib> in C/C++ output.
        #[arg(long = "lib", value_name = "HEADER")]
        libs: Vec<String>,
        /// Where to create the project (default: current directory)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Transpile a Bullang project folder OR a single .bu file.
    ///
    /// Examples:
    ///
    ///   bullang convert my_project          (uses #lang from inventory, default: rs)
    ///
    ///   bullang convert my_project -e py    (explicit target language)
    ///
    ///   bullang convert path/to/file.bu     (single file → stdout)
    ///
    ///   bullang convert path/to/file.bu -o out.rs  (single file → file)
    Convert {
        /// Path to a Bullang project folder or a single .bu source file
        folder: Option<PathBuf>,
        /// Output folder name (project mode only)
        #[arg(short = 'n', long)]
        name: Option<String>,
        /// Target language extension: rs, py, c, cpp, go (default from #lang or rs)
        #[arg(short = 'e', long, default_value = "rs")]
        ext: String,
        /// Explicit output path (project mode only)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Output file (single-file mode only; omit to write to stdout)
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },

    /// Validate and type-check the project from the current directory.
    Check,

    /// Explore the standard library of builtin functions.
    Stdlib {
        #[arg(long)]
        list: bool,
    },

    /// Update bullang to the latest version from the source repository.
    ///
    /// Requires git and cargo to be available on PATH.
    Update,

    /// Start the Bullang language server (LSP) on stdin/stdout.
    ///
    /// Configure your editor to run: bullang lsp
    ///
    /// Capabilities: diagnostics, hover (signatures), go-to-definition.
    Lsp,

    /// Write LSP configuration files for detected editors.
    ///
    /// Supports: Neovim (nvim-lspconfig), Helix, Emacs (eglot).
    /// For VS Code: install the .vsix from the Bullang releases page.
    EditorSetup,


}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Install                               => cmd_install(),
        Command::Init { name, depth, blueprint, lang, libs, path } => cmd_init(name, depth, blueprint, lang, libs, path),
        Command::Convert { folder, name, ext, out, output } => cmd_convert(folder, name, ext, out, output),
        Command::Check                                => cmd_check(),
        Command::Update                                => cmd_update(),
        Command::Stdlib { list }                      => cmd_stdlib(list),
        Command::Lsp                                   => run_lsp(),
        Command::EditorSetup                           => cmd_editor_setup(),

    }
}

// ── install ───────────────────────────────────────────────────────────────────

fn cmd_install() {
    println!("Installing bullang via cargo...");
    let status = std::process::Command::new("cargo")
        .args(["install", "--path", "."])
        .status();

    if let Ok(s) = status {
        if s.success() { println!("Installed to ~/.cargo/bin"); }
    }
}

fn cmd_update() {
    println!("Updating bullang via cargo...");
    let status = std::process::Command::new("cargo")
        .args(["install", "--git", DEFAULT_REPO, "bullang"])
        .status();

    if let Ok(s) = status {
        if s.success() { println!("Update complete."); }
    }
}

// ── init ──────────────────────────────────────────────────────────────────────

fn cmd_init(name: String, depth: u8, blueprint: Option<PathBuf>, lang: Option<String>, libs: Vec<String>, path: Option<PathBuf>) {
    let parent = path.unwrap_or_else(current_dir);

    // ── Blueprint mode ────────────────────────────────────────────────────────
    if let Some(ref bp_path) = blueprint {
        let bp_src = std::fs::read_to_string(bp_path).unwrap_or_else(|e| {
            eprintln!("error: cannot read blueprint file '{}': {}", bp_path.display(), e);
            std::process::exit(1);
        });

        let nodes = init::parse_blueprint(&bp_src).unwrap_or_else(|e| {
            eprintln!("error parsing blueprint: {}", e);
            std::process::exit(1);
        });

        println!("bullang init");
        println!("  name      : {}", name);
        println!("  blueprint : {}", bp_path.display());
        if let Some(ref l) = lang { println!("  lang      : {}", l); }
        println!();

        match init::init_from_blueprint(&parent, &name, &nodes, lang.as_deref(), &bp_src) {
            Ok(result) => {
                init::print_blueprint_tree(&result);
                println!();
                println!("project ready.");
            }
            Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
        }
        return;
    }

    // ── Standard depth-based mode ─────────────────────────────────────────────
    if depth < 1 || depth > 6 {
        eprintln!("error: --depth must be between 1 and 6");
        eprintln!();
        eprintln!("  depth 1 → skirmish");
        eprintln!("  depth 2 → tactic → skirmish");
        eprintln!("  depth 3 → strategy → tactic → skirmish");
        eprintln!("  depth 4 → battle → strategy → tactic → skirmish");
        eprintln!("  depth 5 → theater → battle → strategy → tactic → skirmish");
        eprintln!("  depth 6 → war → theater → battle → strategy → tactic → skirmish");
        std::process::exit(1);
    }

    let root_rank = init::rank_for_depth(depth).unwrap();
    println!("bullang init");
    println!("  name  : {}", name);
    println!("  depth : {} (root rank: {})", depth, root_rank.name());
    if let Some(ref l) = lang {
        println!("  lang  : {}", l);
    }
    if !libs.is_empty() {
        println!("  libs  : {}", libs.join(", "));
    }
    println!();

    match init::init(&parent, &name, depth, lang.as_deref(), &libs) {
        Ok(result) => {
            init::print_tree(&result);
            println!();
            println!("project ready.");
        }
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    }
}

// ── editor-setup ─────────────────────────────────────────────────────────────

fn cmd_editor_setup() {
    use std::io::Write;
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut configured = 0usize;
    let mut skipped    = 0usize;

    println!("bullang editor-setup");
    println!();

    // ── Neovim ───────────────────────────────────────────────────────────────
    let nvim_dir = std::path::PathBuf::from(&home).join(".config").join("nvim");
    if nvim_dir.exists() {
        let ftdetect = nvim_dir.join("after").join("ftdetect");
        std::fs::create_dir_all(&ftdetect).ok();
        let ftfile = ftdetect.join("bullang.vim");
        if !ftfile.exists() {
            let _ = std::fs::write(&ftfile,
                "au BufRead,BufNewFile *.bu set filetype=bullang\n");
            println!("  [Neovim] wrote {}", ftfile.display());
            configured += 1;
        } else {
            println!("  [Neovim] {} exists \u{2014} skipped", ftfile.display());
            skipped += 1;
        }

        let plugin_dir = nvim_dir.join("after").join("plugin");
        std::fs::create_dir_all(&plugin_dir).ok();
        let lua_file = plugin_dir.join("bullang.lua");
        if !lua_file.exists() {
            let lines: &[&str] = &[
                "-- Bullang LSP (auto-generated by `bullang editor-setup`)",
                r#"local ok, lspconfig = pcall(require, "lspconfig")"#,
                "if not ok then return end",
                r#"local configs = require("lspconfig.configs")"#,
                "if not configs.bullang then",
                "  configs.bullang = {",
                "    default_config = {",
                r#"      cmd = { "bullang", "lsp" },"#,
                r#"      filetypes = { "bullang" },"#,
                r#"      root_dir = lspconfig.util.root_pattern("inventory.bu"),"#,
                "    },",
                "  }",
                "end",
                "lspconfig.bullang.setup({})",
                "",
            ];
            let lua = lines.join("\n");
            let _ = std::fs::write(&lua_file, lua);
            println!("  [Neovim] wrote {}", lua_file.display());
            println!("           (requires nvim-lspconfig)");
            configured += 1;
        } else {
            println!("  [Neovim] {} exists \u{2014} skipped", lua_file.display());
            skipped += 1;
        }
    } else {
        println!("  [Neovim] not detected (~/.config/nvim not found)");
    }
    println!();

    // ── Helix ────────────────────────────────────────────────────────────────
    let helix_dir = std::path::PathBuf::from(&home).join(".config").join("helix");
    if helix_dir.exists() {
        let lang_file = helix_dir.join("languages.toml");
        let existing  = std::fs::read_to_string(&lang_file).unwrap_or_default();
        if existing.contains(r#"name = "bullang""#) {
            println!("  [Helix] bullang already in languages.toml \u{2014} skipped");
            skipped += 1;
        } else {
            let lines: &[&str] = &[
                "",
                "[[language]]",
                r#"name = "bullang""#,
                r#"scope = "source.bullang""#,
                r#"file-types = ["bu"]"#,
                r#"comment-token = "//""#,
                r#"language-servers = ["bullang-lsp"]"#,
                "",
                "[language-server.bullang-lsp]",
                r#"command = "bullang""#,
                r#"args = ["lsp"]"#,
                "",
            ];
            let entry = lines.join("\n");
            let mut f = std::fs::OpenOptions::new()
                .create(true).append(true).open(&lang_file)
                .expect("cannot open languages.toml");
            let _ = f.write_all(entry.as_bytes());
            println!("  [Helix] appended to {}", lang_file.display());
            configured += 1;
        }
    } else {
        println!("  [Helix] not detected (~/.config/helix not found)");
    }
    println!();

    // ── Emacs (eglot) ────────────────────────────────────────────────────────
    let emacs_dir = std::path::PathBuf::from(&home).join(".emacs.d");
    if emacs_dir.exists() {
        let el_file = emacs_dir.join("bullang-lsp.el");
        if !el_file.exists() {
            let lines: &[&str] = &[
                ";; Bullang LSP (auto-generated by `bullang editor-setup`)",
                r#";; Add to init.el: (load (expand-file-name "bullang-lsp.el" user-emacs-directory))"#,
                "(require 'eglot)",
                r#"(add-to-list 'auto-mode-alist '("\\.bu\\'" . prog-mode))"#,
                "(add-to-list 'eglot-server-programs",
                r#"             '(prog-mode . ("bullang" "lsp")))"#,
                "(add-hook 'prog-mode-hook",
                "          (lambda ()",
                "            (when (and buffer-file-name",
                r#"                       (string-suffix-p ".bu" buffer-file-name))"#,
                "              (eglot-ensure))))",
                "",
            ];
            let el = lines.join("\n");
            let _ = std::fs::write(&el_file, el);
            println!("  [Emacs] wrote {}", el_file.display());
            println!(r#"          add to init.el: (load (expand-file-name "bullang-lsp.el" user-emacs-directory))"#);
            configured += 1;
        } else {
            println!("  [Emacs] {} exists \u{2014} skipped", el_file.display());
            skipped += 1;
        }
    } else {
        println!("  [Emacs] not detected (~/.emacs.d not found)");
    }
    println!();

    // ── VS Code ───────────────────────────────────────────────────────────────
    println!("  [VS Code] install the extension (.vsix) from:");
    println!("            https://github.com/My-sidequests/Bullang/releases");
    println!();
    println!("configured: {}   skipped (already set up): {}", configured, skipped);
}

// ── lsp ──────────────────────────────────────────────────────────────────────

fn run_lsp() {
    lsp::run();
}

// ── stdlib ───────────────────────────────────────────────────────────────────

fn cmd_stdlib(_list: bool) {
    println!("Bullang standard library — 13 universal builtins");
    println!("Available in every backend: Rust, Python, C, C++, Go");
    println!();

    println!("  Math");
    println!("  ----");
    let math = ["abs","pow","powf","sqrt","clamp"];
    let builtins = stdlib::list_builtins();
    for (name, sig, desc) in &builtins {
        if math.contains(name) {
            println!("    builtin::{:<14}  {}  — {}", name, sig, desc);
        }
    }
    println!();
    println!("  String");
    println!("  ------");
    for (name, sig, desc) in &builtins {
        if !math.contains(name) {
            println!("    builtin::{:<14}  {}  — {}", name, sig, desc);
        }
    }
    println!();
    println!("Usage in a source file:");
    println!();
    println!("  let upper(s: String) -> result: String {{");
    println!("      builtin::to_upper");
    println!("  }}");
    println!();
    println!("  let absolute(x: i32) -> result: i32 {{");
    println!("      builtin::abs");
    println!("  }}");
    println!();
    println!("The function's declared parameters are passed to the builtin in order.");
    println!("Parameter counts are enforced at build time.");
}

// ── convert ───────────────────────────────────────────────────────────────────

fn cmd_convert(folder: Option<PathBuf>, name: Option<String>, ext: String, out: Option<PathBuf>, output: Option<PathBuf>) {
    // ── Single-file mode ──────────────────────────────────────────────────────
    // Detect single .bu file by extension first, before canonicalize().
    if let Some(ref p) = folder {
        let is_bu = p.extension().map(|e| e == "bu").unwrap_or(false);
        if is_bu {
            // Resolve path; if not found give a clear error
            let resolved = if p.exists() {
                p.canonicalize().unwrap_or_else(|_| p.clone())
            } else {
                eprintln!("error: '{}' not found", p.display());
                std::process::exit(1);
            };
            cmd_convert_file(resolved, ext, output);
            return;
        }
    }

    // If -e was left at the default "rs", check whether the project declares #lang
    // in its inventory — if so, honour that instead.
    let resolved_ext = if ext == "rs" {
        // Peek at the root inventory before we fully parse it
        let probe_dir = match &folder {
            Some(p) => p.canonicalize().unwrap_or_else(|_| p.clone()),
            None    => current_dir(),
        };
        let probe_root = find_root_from_probe(&probe_dir);
        if let Ok(inv) = validator::read_inventory(&probe_root) {
            if let Some(ref lang) = inv.lang {
                lang.ext().to_string()
            } else {
                ext.clone()
            }
        } else {
            ext.clone()
        }
    } else {
        ext.clone()
    };

    let backend = Backend::from_ext(&resolved_ext).unwrap_or_else(|| {
        eprintln!("error: unknown extension '{}' — supported: rs, py, c, cpp, go", resolved_ext);
        std::process::exit(1);
    });

    let source_dir = match folder {
        Some(ref p) => {
            let c = p.canonicalize().unwrap_or_else(|_| p.clone());
            if !c.is_dir() { eprintln!("error: '{}' is not a directory", p.display()); std::process::exit(1); }
            c
        }
        None => current_dir(),
    };

    let root = find_root_from(&source_dir);

    let source_name = source_dir.file_name()
        .and_then(|n| n.to_str()).unwrap_or("bullang_project").to_string();

    let out_dir = match out {
        Some(p) => p,
        None => {
            let out_name = name.unwrap_or_else(|| format!("_{}", source_name));
            source_dir.parent().unwrap_or(&source_dir).join(out_name)
        }
    };

    if out_dir.starts_with(&root) || root.starts_with(&out_dir) {
        eprintln!("error: output must be outside the source tree");
        std::process::exit(1);
    }

    let crate_name = out_dir.file_name()
        .and_then(|n| n.to_str()).unwrap_or("bullang_out").to_string();
    let root_rank  = validator::read_folder_rank(&root).expect("root has no rank");

    println!("bullang convert");
    println!("  source  : {} ({})", root.display(), root_rank.name());
    println!("  output  : {}", out_dir.display());
    println!("  crate   : {}", crate_name);
    println!("  backend : {}", backend.name());
    println!();

    let all_errors = validator::validate_tree(&root);
    if !all_errors.is_empty() {
        print_all_errors(&all_errors);
        std::process::exit(1);
    }
    println!("structural validation ... ok");

    // Backend compatibility: reject escape blocks targeting a different backend
    let compat_errors = build::validate_backend_compatibility(&root, &backend);
    if !compat_errors.is_empty() {
        let all = AllErrors { parse: vec![], structural: compat_errors };
        print_all_errors(&all);
        std::process::exit(1);
    }

    let type_errors = typecheck::typecheck_tree(&root);
    if !type_errors.is_empty() {
        print_type_errors(&type_errors);
        std::process::exit(1);
    }
    println!("type checking         ... ok");

    let result = build::build(&root, &out_dir, &crate_name, &backend);
    if !result.errors.is_empty() {
        let all = AllErrors { parse: vec![], structural: result.errors };
        print_all_errors(&all);
        eprintln!("\nconvert failed");
        std::process::exit(1);
    }

    println!("code generation       ... ok");
    println!();
    println!("wrote {} file(s) to {}", result.files_written, out_dir.display());
    println!();
    match backend {
        Backend::Rust => {
            println!("to compile:");
            println!("  cd {} && cargo build", out_dir.display());
        }
        Backend::Python => {
            println!("to run:");
            println!("  cd {} && python3 -m {}", out_dir.display(), crate_name);
        }
        Backend::C => {
            println!("to compile:");
            println!("  cd {} && make", out_dir.display());
        }
        Backend::Cpp => {
            println!("to compile:");
            println!("  cd {} && make", out_dir.display());
        }
        Backend::Go => {
            println!("to run:");
            println!("  cd {} && go run .", out_dir.display());
        }
        Backend::Unknown(kw) => {
            eprintln!("error: unknown backend '{}'", kw);
        }
    }
}

// ── check ─────────────────────────────────────────────────────────────────────

fn cmd_check() {
    let root = find_root_from(&current_dir());
    let rank = validator::read_folder_rank(&root).expect("root has no rank");

    println!("bullang check");
    println!("  root : {} ({})", root.display(), rank.name());
    println!();

    let all_errors = validator::validate_tree(&root);
    if !all_errors.is_empty() {
        print_all_errors(&all_errors);
        std::process::exit(1);
    }

    let type_errors = typecheck::typecheck_tree(&root);
    if type_errors.is_empty() {
        println!("ok -- no errors found");
    } else {
        print_type_errors(&type_errors);
        std::process::exit(1);
    }
}

// ── convert single file ──────────────────────────────────────────────────────
// `bullang convert path/to/file.bu [-e ext] [-o out]`
// Transpiles one source file without tree context.

fn cmd_convert_file(input: PathBuf, ext: String, output: Option<PathBuf>) {
    let source = read_file(&input);
    let is_inv = input.file_name().and_then(|n| n.to_str())
        .map(|n| n == "inventory.bu").unwrap_or(false);

    let bu = parser::parse_file(&source, is_inv).unwrap_or_else(|e| {
        eprintln!("parse error in {}:\n  {}", input.display(), e);
        std::process::exit(1);
    });

    let backend = Backend::from_ext(&ext).unwrap_or(Backend::Rust);

    match bu {
        ast::BuFile::Source(ref sf) => {
            use std::collections::HashSet;
            let path   = input.display().to_string();
            let errors = validator::validate_source_direct(
                sf, &path, &HashSet::new(), &ast::Rank::Skirmish,
            );
            if !errors.is_empty() {
                let all = AllErrors { parse: vec![], structural: errors };
                print_all_errors(&all);
                std::process::exit(1);
            }
            let type_errors = typecheck::typecheck_file(sf, &path);
            if !type_errors.is_empty() {
                print_type_errors(&type_errors);
                std::process::exit(1);
            }
            let content = match backend {
                Backend::Rust        => codegen::emit_source(sf),
                Backend::Python      => codegen_python::emit_source_py(sf),
                Backend::C           => {
                    let hdr = format!("{}.h", input.file_stem()
                        .and_then(|s| s.to_str()).unwrap_or("out"));
                    codegen_c::emit_source_c(sf, &hdr)
                }
                Backend::Cpp         => {
                    let hdr = format!("{}.hpp", input.file_stem()
                        .and_then(|s| s.to_str()).unwrap_or("out"));
                    codegen_cpp::emit_source_cpp(sf, &hdr)
                }
                Backend::Go          => codegen_go::emit_source_go(sf, "main"),
                Backend::Unknown(_)  => codegen::emit_source(sf),
            };
            write_or_print(content, output);
        }
        ast::BuFile::Inventory(_) => {
            write_or_print(codegen::emit_mod_rs(&[]), output);
        }
    }
}

// ── Error display ─────────────────────────────────────────────────────────────

fn print_all_errors(all: &AllErrors) {
    let mut by_file: BTreeMap<String, Vec<(usize, usize, String)>> = BTreeMap::new();

    for e in &all.parse {
        by_file.entry(e.file.clone()).or_default()
            .push((e.line, e.col, format!("parse error: {}", e.message)));
    }
    for e in &all.structural {
        by_file.entry(e.file.clone()).or_default()
            .push((e.line, e.col, e.message.clone()));
    }

    let mut total = 0;
    let file_count = by_file.len();

    for (file, mut entries) in by_file {
        entries.sort_by_key(|(line, col, _)| (*line, *col));
        eprintln!();
        eprintln!("  {}:", file);
        for (line, col, msg) in &entries {
            total += 1;
            if *line > 0 { eprintln!("    [{}:{}] {}", line, col, msg); }
            else         { eprintln!("    {}", msg); }
        }
    }

    eprintln!();
    eprintln!("{} error(s) in {} file(s)", total, file_count);
}

fn print_type_errors(errors: &[typecheck::TypeError]) {
    let mut by_file: BTreeMap<String, Vec<(usize, usize, String)>> = BTreeMap::new();

    for e in errors {
        by_file.entry(e.file.clone()).or_default()
            .push((e.line, e.col, e.message.clone()));
    }

    let mut total = 0;
    let file_count = by_file.len();

    for (file, mut entries) in by_file {
        entries.sort_by_key(|(line, col, _)| (*line, *col));
        eprintln!();
        eprintln!("  {}:", file);
        for (line, col, msg) in &entries {
            total += 1;
            if *line > 0 { eprintln!("    [{}:{}] type error: {}", line, col, msg); }
            else         { eprintln!("    type error: {}", msg); }
        }
    }

    eprintln!();
    eprintln!("{} type error(s) in {} file(s)", total, file_count);
}

// ── Root detection (probe — no exit on failure) ──────────────────────────────

/// Like find_root_from but returns the given dir if no inventory found (no exit).
fn find_root_from_probe(start: &Path) -> PathBuf {
    if !start.join("inventory.bu").exists() { return start.to_path_buf(); }
    let mut root = start.to_path_buf();
    loop {
        match root.parent() {
            Some(p) if p.join("inventory.bu").exists() => root = p.to_path_buf(),
            _ => break,
        }
    }
    root
}

// ── Root detection ────────────────────────────────────────────────────────────

fn find_root_from(start: &Path) -> PathBuf {
    if !start.join("inventory.bu").exists() {
        eprintln!(
            "error: no inventory.bu in '{}'\n\
             run bullang from inside a Bullang project folder",
            start.display()
        );
        std::process::exit(1);
    }
    let mut root = start.to_path_buf();
    loop {
        match root.parent() {
            Some(p) if p.join("inventory.bu").exists() => root = p.to_path_buf(),
            _ => break,
        }
    }
    if validator::read_folder_rank(&root).is_none() {
        eprintln!("error: could not read #rank from '{}/inventory.bu'", root.display());
        std::process::exit(1);
    }
    root
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("error: {}", e); std::process::exit(1);
    })
}

fn read_file(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error reading {}: {}", path.display(), e); std::process::exit(1);
    })
}

fn write_or_print(content: String, output: Option<PathBuf>) {
    match output {
        Some(ref p) => std::fs::write(p, &content).unwrap_or_else(|e| {
            eprintln!("error writing {}: {}", p.display(), e); std::process::exit(1);
        }),
        None => print!("{}", content),
    }
}
