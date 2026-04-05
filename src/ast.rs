// ── Rank ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Rank {
    War,
    Theater,
    Battle,
    Strategy,
    Tactic,
    Skirmish,
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

    pub fn expected_depth(&self) -> usize {
        match self {
            Rank::War      => 0,
            Rank::Theater  => 1,
            Rank::Battle   => 2,
            Rank::Strategy => 3,
            Rank::Tactic   => 4,
            Rank::Skirmish => 5,
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

#[derive(Debug, Clone)]
pub enum BuType {
    /// Any plain Rust type: i16, bool, String, Vec<i16>, etc.
    Generic(String),
    /// (T, U, ...) tuple
    Tuple(Vec<BuType>),
    /// [T; N] array
    Array(Box<BuType>, usize),
}

impl BuType {
    /// Emit the type as a valid Rust type string
    pub fn to_rust(&self) -> String {
        match self {
            BuType::Generic(s)     => s.clone(),
            BuType::Tuple(inner)   => format!(
                "({})",
                inner.iter().map(|t| t.to_rust()).collect::<Vec<_>>().join(", ")
            ),
            BuType::Array(ty, n)   => format!("[{}; {}]", ty.to_rust(), n),
        }
    }
}

// ── Expressions ───────────────────────────────────────────────────────────────

/// A call argument — either a plain ident (value) or &ident (bullet reference)
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
    /// Tuple value: (expr, expr, ...)
    Tuple(Vec<Expr>),
}

// ── Pipe ──────────────────────────────────────────────────────────────────────

/// (inputs) : expr -> {binding};
#[derive(Debug, Clone)]
pub struct Pipe {
    pub inputs:  Vec<String>,
    pub expr:    Expr,
    pub binding: String,   // the name inside {}
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
    pub pipes:    Vec<Pipe>,
    pub exported: bool,
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
