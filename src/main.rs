mod ast;
mod build;
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
    version = "0.1.0",
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
    ///   bullang init my_project --depth 2
    ///   bullang init my_project --depth 4 --lib stdio.h --lib math.h
    Init {
        /// Name of the project folder to create
        name: String,
        /// Hierarchy depth: 1 = skirmish, 2 = tactic+skirmish, … 6 = full war chain
        #[arg(short, long, default_value = "2")]
        depth: u8,
        /// External library to declare (repeatable). Used as #include <lib> in C/C++ output.
        /// Example: --lib stdio.h --lib math.h
        #[arg(long = "lib", value_name = "HEADER")]
        libs: Vec<String>,
        /// Where to create the project (default: current directory)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Transpile a Bullang project folder.
    Convert {
        folder: Option<PathBuf>,
        #[arg(short = 'n', long)]
        name: Option<String>,
        #[arg(short = 'e', long, default_value = "rs")]
        ext: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Validate and type-check the project from the current directory.
    Check,

    /// Explore the standard library of builtin functions.
    Stdlib {
        /// List all available builtin functions and their signatures
        #[arg(long)]
        list: bool,
    },

    /// Transpile a single .bu file to stdout or --output.
    File {
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Install                               => cmd_install(),
        Command::Init { name, depth, libs, path }     => cmd_init(name, depth, libs, path),
        Command::Convert { folder, name, ext, out }   => cmd_convert(folder, name, ext, out),
        Command::Check                                => cmd_check(),
        Command::Stdlib { list }                      => cmd_stdlib(list),
        Command::File { input, output }               => cmd_file(input, output),
    }
}

// ── install ───────────────────────────────────────────────────────────────────

fn cmd_install() {
    let current_exe = std::env::current_exe().unwrap_or_else(|e| {
        eprintln!("error: could not locate binary: {}", e);
        std::process::exit(1);
    });

    let user_local = format!(
        "{}/.local/bin/bullang",
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    );
    let dest = find_install_dest(&["/usr/local/bin/bullang", "/usr/bin/bullang"], &user_local);

    if let Some(parent) = Path::new(&dest).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("error: could not create {}: {}", parent.display(), e);
            std::process::exit(1);
        }
    }

    match std::fs::copy(&current_exe, &dest) {
        Ok(_) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest).unwrap().permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest, perms).ok();
            }
            println!("installed: {}", dest);
            println!("bullang is now available globally.");
            check_path_contains(&dest);
        }
        Err(e) => {
            eprintln!("error: could not install to {}: {}", dest, e);
            eprintln!("try: sudo bullang install");
            std::process::exit(1);
        }
    }
}

fn find_install_dest(system_paths: &[&str], user_fallback: &str) -> String {
    for path in system_paths {
        let dir = Path::new(path).parent().unwrap_or(Path::new("/usr/local/bin"));
        if is_writable(dir) { return path.to_string(); }
    }
    user_fallback.to_string()
}

fn is_writable(path: &Path) -> bool {
    if !path.exists() { return false; }
    let test = path.join(".bullang_write_test");
    match std::fs::write(&test, b"") {
        Ok(_) => { std::fs::remove_file(test).ok(); true }
        Err(_) => false,
    }
}

fn check_path_contains(dest: &str) {
    let dest_dir = Path::new(dest).parent()
        .map(|p| p.display().to_string()).unwrap_or_default();
    let in_path = std::env::var("PATH").unwrap_or_default()
        .split(':').any(|p| p == dest_dir);
    if !in_path {
        println!();
        println!("note: {} is not in your PATH.", dest_dir);
        println!("add to your shell profile:");
        println!("  export PATH=\"{}:$PATH\"", dest_dir);
    }
}

// ── init ──────────────────────────────────────────────────────────────────────

fn cmd_init(name: String, depth: u8, libs: Vec<String>, path: Option<PathBuf>) {
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

    let parent = path.unwrap_or_else(current_dir);

    let root_rank = init::rank_for_depth(depth).unwrap();
    println!("bullang init");
    println!("  name  : {}", name);
    println!("  depth : {} (root rank: {})", depth, root_rank.name());
    if !libs.is_empty() {
        println!("  libs  : {}", libs.join(", "));
    }
    println!();

    match init::init(&parent, &name, depth, &libs) {
        Ok(result) => {
            init::print_tree(&result);
            println!();
            println!("project ready. next steps:");
            println!("  cd {}", result.root.display());
            if depth > 1 {
                println!("  # edit main.bu to write your entry point");
            }
            println!("  # edit the .bu files in the skirmish folder");
            println!("  bullang check");
            println!("  bullang convert {} -n {}_out", name, name);
        }
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    }
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

fn cmd_convert(folder: Option<PathBuf>, name: Option<String>, ext: String, out: Option<PathBuf>) {
    let backend = Backend::from_ext(&ext).unwrap_or_else(|| {
        eprintln!("error: unknown extension '{}' — supported: rs, py, c, cpp, go", ext);
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

// ── file ──────────────────────────────────────────────────────────────────────

fn cmd_file(input: PathBuf, output: Option<PathBuf>) {
    let source = read_file(&input);
    let is_inv = input.file_name().and_then(|n| n.to_str())
        .map(|n| n == "inventory.bu").unwrap_or(false);

    let bu = parser::parse_file(&source, is_inv).unwrap_or_else(|e| {
        eprintln!("parse error in {}:\n  {}", input.display(), e);
        std::process::exit(1);
    });

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
            write_or_print(codegen::emit_source(sf), output);
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
