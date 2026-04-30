use std::collections::HashMap;

// ── Source location ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col:  usize,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self { Self { line, col } }
}

// ── Backend ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Backend {
    Rust,
    Python,
    C,
    Cpp,
    Go,
    /// An unrecognised backend name written in an escape block.
    Unknown(String),
}

impl Backend {
    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            "rs"  => Some(Backend::Rust),
            "py"  => Some(Backend::Python),
            "c"   => Some(Backend::C),
            "cpp" | "cc" | "cxx" => Some(Backend::Cpp),
            "go"  => Some(Backend::Go),
            _     => None,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Backend::Rust       => "rust",
            Backend::Python     => "python",
            Backend::C          => "c",
            Backend::Cpp        => "cpp",
            Backend::Go         => "go",
            Backend::Unknown(_) => "unknown",
        }
    }
    pub fn ext(&self) -> &'static str {
        match self {
            Backend::Rust       => "rs",
            Backend::Python     => "py",
            Backend::C          => "c",
            Backend::Cpp        => "cpp",
            Backend::Go         => "go",
            Backend::Unknown(_) => "?",
        }
    }
    pub fn escape_keyword(&self) -> String {
        match self {
            Backend::Rust        => "rust".to_string(),
            Backend::Python      => "python".to_string(),
            Backend::C           => "c".to_string(),
            Backend::Cpp         => "cpp".to_string(),
            Backend::Go          => "go".to_string(),
            Backend::Unknown(s)  => s.clone(),
        }
    }
}

// ── Rank ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    Skirmish,
    Tactic,
    Strategy,
    Battle,
    Theater,
    War,
}

impl Rank {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "war"      => Some(Rank::War),
            "theater"  => Some(Rank::Theater),
            "battle"   => Some(Rank::Battle),
            "strategy" => Some(Rank::Strategy),
            "tactic"   => Some(Rank::Tactic),
            "skirmish" => Some(Rank::Skirmish),
            _          => None,
        }
    }

    pub fn child_rank(&self) -> Option<Rank> {
        match self {
            Rank::War      => Some(Rank::Theater),
            Rank::Theater  => Some(Rank::Battle),
            Rank::Battle   => Some(Rank::Strategy),
            Rank::Strategy => Some(Rank::Tactic),
            Rank::Tactic   => Some(Rank::Skirmish),
            Rank::Skirmish => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Rank::War      => "war",
            Rank::Theater  => "theater",
            Rank::Battle   => "battle",
            Rank::Strategy => "strategy",
            Rank::Tactic   => "tactic",
            Rank::Skirmish => "skirmish",
        }
    }

    pub fn has_own_files(&self) -> bool  { *self != Rank::War }
    pub fn has_sub_folders(&self) -> bool { *self != Rank::Skirmish }
}

// ── Type system ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BuType {
    Named(String),
    Tuple(Vec<BuType>),
    Array(Box<BuType>, usize),
    Unknown,
}

impl BuType {
    pub fn to_rust(&self) -> String {
        match self {
            BuType::Named(s)     => s.clone(),
            BuType::Tuple(inner) => format!(
                "Tuple[{}]",
                inner.iter().map(|t| t.to_rust()).collect::<Vec<_>>().join(", ")
            ),
            BuType::Array(ty, n) => format!("[{}; {}]", ty.to_rust(), n),
            BuType::Unknown      => "_".to_string(),
        }
    }

    pub fn is_numeric(&self) -> bool {
        match self {
            BuType::Named(s) => matches!(
                s.as_str(),
                "i8"|"i16"|"i32"|"i64"|"i128"|"isize"|
                "u8"|"u16"|"u32"|"u64"|"u128"|"usize"|
                "f32"|"f64"
            ),
            _ => false,
        }
    }
}

// ── Type environment ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BulletSig {
    pub params:  Vec<BuType>,
    pub returns: BuType,
}

pub type TypeEnv = HashMap<String, BulletSig>;

// ── Expressions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CallArg {
    Value(String),
    BulletRef(String),
}

#[derive(Debug, Clone)]
pub enum Atom {
    Ident(String),
    Integer(i64),
    /// A plain string literal with no interpolation: `"hello"`
    StringLit(String),
    /// A string template with `{var}` placeholders: `"hello {name}!"`
    /// Stored as the raw template content (quotes stripped).
    /// Each codegen resolves the placeholders into its own format mechanism.
    Interp(String),
    Call { name: String, args: Vec<CallArg> },
}

#[derive(Debug, Clone)]
pub struct BinExpr {
    pub lhs: Atom,
    pub op:  String,
    pub rhs: Atom,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Atom(Atom),
    BinOp(BinExpr),
    Tuple(Vec<Expr>),
}

// ── Pipe ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Pipe {
    pub inputs:  Vec<String>,
    pub expr:    Expr,
    pub binding: String,
    pub span:    Span,
}

// ── Bullet body ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BulletBody {
    /// Pure Bullang pipe chain.
    Pipes(Vec<Pipe>),
    /// Verbatim code block for a specific backend (`@rust ... @end`).
    Native { backend: Backend, code: String },
    /// Reference to a stdlib builtin (`@builtin name`).
    /// Resolved at codegen time from the backend's stdlib table.
    Builtin(String),
}

// ── Output declaration ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OutputDecl {
    pub name: String,
    pub ty:   BuType,
}

// ── Parameter ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty:   BuType,
}

// ── Struct definitions ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty:   BuType,
}

/// A struct type definition. Structs are always public.
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name:   String,
    pub fields: Vec<StructField>,
}

// ── Bullet ────────────────────────────────────────────────────────────────────

/// A single function. All bullets are always public — there is no private code.
#[derive(Debug, Clone)]
pub struct Bullet {
    pub name:    String,
    pub params:  Vec<Param>,
    pub output:  OutputDecl,
    pub body:    BulletBody,
    pub span:    Span,
    /// True when the function is annotated with `#test` — only compiled by `bullang test`.
    pub is_test: bool,
}

// ── Inventory entry ───────────────────────────────────────────────────────────

/// One line in inventory.bu: `filename : fn1, fn2, fn3;`
#[derive(Debug, Clone)]
pub struct InventoryEntry {
    pub file:      String,        // filename without .bu extension
    pub functions: Vec<String>,   // all functions declared in that file
}

// ── File types ────────────────────────────────────────────────────────────────

/// A source .bu file — only bullet declarations. Structs live in inventory.bu.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub bullets: Vec<Bullet>,
}

/// An inventory.bu file — rank, directives, struct definitions, and file manifest.
#[derive(Debug, Clone)]
pub struct InventoryFile {
    pub rank:    Rank,
    pub lang:    Option<Backend>,      // #lang: ext;
    pub libs:    Vec<String>,          // #lib: header; (C/C++ only)
    pub structs: Vec<StructDef>,       // struct definitions for this folder
    pub entries: Vec<InventoryEntry>,  // one per source file in this folder
}

#[derive(Debug, Clone)]
pub enum BuFile {
    Source(SourceFile),
    Inventory(InventoryFile),
}
