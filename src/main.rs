mod ast;
mod build;
mod lsp;
mod codegen;
mod init;
mod parser;
mod stdlib;
mod typecheck;
mod validator;
mod cmd;
mod fmt;
mod readme;
mod utils;

use clap::{Parser as ClapParser, Subcommand};
use std::path::PathBuf;

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
        #[arg(short = 'e', long)]
        ext: Option<String>,
        /// Explicit output path (project mode only)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Output file (single-file mode only; omit to write to stdout)
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },

    /// Strip test folders from a converted output tree for production.
    ///
    /// Deletes every folder whose name starts with `test_`, recursively.
    /// Restarts from the root after each deletion so no test folder is missed.
    ///
    /// Example:
    ///
    ///   bullang prod my_c_project
    Prod {
        /// Path to the output folder to clean
        folder: PathBuf,
    },

    /// Format all .bu files in the project to canonical style.    ///
    /// Rewrites files in place. Escape block contents are never modified.
    /// To check formatting without writing, use `bullang check`.
    ///
    /// Examples:
    ///
    ///   bullang fmt                   (formats project from current directory)
    ///
    ///   bullang fmt my_project        (formats specific project folder)
    ///
    ///   bullang fmt --dry-run         (show what would change without writing)
    Fmt {
        /// Path to the project to format (default: current directory)
        folder: Option<PathBuf>,
        /// Show files that would be reformatted without writing anything
        #[arg(long)]
        dry_run: bool,
    },

    /// Validate and type-check the project from the current directory.
    /// Also reports any files not in canonical format — run `bullang fmt` to fix.
    Check,

    /// Explore the standard library of builtin functions.
    Stdlib {
        #[arg(long)]
        list: bool,
    },

    /// Update bullang to the latest version from the source repository.
    ///
    /// Requires git and cargo to be available on PATH.
    ///
    /// Examples:
    ///
    ///   bullang update                  (pulls from main branch)
    ///
    ///   bullang update --experimental   (pulls from the experimental branch)
    Update {
        /// Pull from the experimental branch instead of main.
        /// Running `bullang update` without this flag afterwards reverts to main.
        #[arg(long)]
        experimental: bool,
    },

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
        Command::Install                                               => cmd::cmd_install(),
        Command::Init { name, depth, blueprint, lang, libs, path }    => cmd::cmd_init(name, depth, blueprint, lang, libs, path),
        Command::Convert { folder, name, ext, out, output }  => cmd::cmd_convert(folder, name, ext, out, output),
        Command::Fmt { folder, dry_run }                       => cmd::cmd_fmt(folder, dry_run),
        Command::Prod { folder }                                        => cmd::cmd_prod(folder),
        Command::Check                                                 => cmd::cmd_check(),
        Command::Update { experimental }                               => cmd::cmd_update(experimental),
        Command::Stdlib { list }                                       => cmd::cmd_stdlib(list),
        Command::Lsp                                                   => cmd::run_lsp(),
        Command::EditorSetup                                           => cmd::cmd_editor_setup(),
    }
}
