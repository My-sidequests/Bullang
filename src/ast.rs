use std::collections::HashMap;

// ── Source location ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col:  usize,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self { Self { line, col } }
    pub fn unknown() -> Self { Self { line: 0, col: 0 } }
    pub fn is_known(&self) -> bool { self.line > 0 }
}

// ── Backend ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Backend {
    Rust,
}

impl Backend {
    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext { "rs" => Some(Backend::Rust), _ => None }
    }
    pub fn name(&self) -> &'static str { match self { Backend::Rust => "rust" } }
    pub fn ext(&self)  -> &'static str { match self { Backend::Rust => "rs"   } }
}

// ── Rank ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    Skirmish,   // lowest
    Tactic,
    Strategy,
    Battle,
    Theater,
    War,        // highest
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

    /// The rank of immediate child folders.
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

    /// True if this rank can contain .bu files of its own rank.
    pub fn has_own_files(&self) -> bool {
        *self != Rank::War
    }

    /// True if this rank can contain sub-folders.
    pub fn has_sub_folders(&self) -> bool {
        *self != Rank::Skirmish
    }
}

// ── Category ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Category {
    Algorithm,
    Function,
}

impl Category {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "algorithm" => Some(Category::Algorithm),
            "function"  => Some(Category::Function),
            _           => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Algorithm => "algorithm",
            Category::Function  => "function",
        }
    }
}

// ── Type system ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BuType {
    Named(String),
    Tuple(Vec<BuType>),
    Array(Box<BuType>, usize),
    /// Placeholder when inference cannot determine a type — propagates silently.
    Unknown,
}

impl BuType {
    pub fn to_rust(&self) -> String {
        match self {
            BuType::Named(s)     => s.clone(),
            BuType::Tuple(inner) => format!(
                "({})",
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
    Pipes(Vec<Pipe>),
    Native { backend: Backend, code: String },
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

// ── Bullet ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Bullet {
    pub name:     String,
    pub params:   Vec<Param>,
    pub output:   OutputDecl,
    pub body:     BulletBody,
    pub exported: bool,
    pub span:     Span,
}

// ── File types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SkirmishFile {
    pub rank:     Rank,
    pub category: Category,
    pub imports:  Vec<String>,
    pub exports:  Vec<String>,
    pub bullets:  Vec<Bullet>,
}

#[derive(Debug, Clone)]
pub struct InventoryFile {
    pub rank:    Rank,
    pub exports: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum BuFile {
    Skirmish(SkirmishFile),
    Inventory(InventoryFile),
}
