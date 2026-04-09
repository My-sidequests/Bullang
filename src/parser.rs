use pest::iterators::Pair;
use crate::ast::*;

#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
pub struct BulParser;

// ── Public entry point ────────────────────────────────────────────────────────

pub fn parse_file(
    source:       &str,
    is_inventory: bool,
) -> Result<BuFile, Box<dyn std::error::Error>> {
    use pest::Parser;
    if is_inventory {
        let mut pairs = BulParser::parse(Rule::inventory_file, source)?;
        Ok(BuFile::Inventory(parse_inventory(pairs.next().unwrap())))
    } else {
        let mut pairs = BulParser::parse(Rule::source_file, source)?;
        Ok(BuFile::Source(parse_source(pairs.next().unwrap())))
    }
}

// ── Span extraction ───────────────────────────────────────────────────────────

fn span_of(pair: &Pair<Rule>) -> Span {
    let (line, col) = pair.as_span().start_pos().line_col();
    Span::new(line, col)
}

// ── Source file ───────────────────────────────────────────────────────────────

fn parse_source(pair: Pair<Rule>) -> SourceFile {
    let bullets = pair.into_inner()
        .filter(|p| p.as_rule() == Rule::bullet)
        .map(parse_bullet)
        .collect();
    SourceFile { bullets }
}

// ── Inventory file ────────────────────────────────────────────────────────────

fn parse_inventory(pair: Pair<Rule>) -> InventoryFile {
    let mut rank    = None;
    let mut entries = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::dir_rank => {
                rank = Rank::from_str(inner.into_inner().next().unwrap().as_str());
            }
            Rule::inv_entry => {
                let mut ci    = inner.into_inner();
                let file      = ci.next().unwrap().as_str().to_string();
                let functions = ci.map(|p| p.as_str().to_string()).collect();
                entries.push(InventoryEntry { file, functions });
            }
            Rule::EOI => {}
            _         => {}
        }
    }

    InventoryFile {
        rank:    rank.expect("inventory.bu is missing #rank"),
        entries,
    }
}

// ── Bullet ────────────────────────────────────────────────────────────────────

fn parse_bullet(pair: Pair<Rule>) -> Bullet {
    let bullet_span = span_of(&pair);
    let mut inner   = pair.into_inner();
    let name        = inner.next().unwrap().as_str().to_string();
    let params      = parse_param_list(inner.next().unwrap());
    let output      = parse_output_decl(inner.next().unwrap());
    let body        = parse_bullet_body(inner.next().unwrap());
    Bullet { name, params, output, body, span: bullet_span }
}

fn parse_param_list(pair: Pair<Rule>) -> Vec<Param> {
    pair.into_inner()
        .filter(|p| p.as_rule() == Rule::param)
        .map(|p| {
            let mut pi = p.into_inner();
            Param {
                name: pi.next().unwrap().as_str().to_string(),
                ty:   parse_ty(pi.next().unwrap()),
            }
        })
        .collect()
}

fn parse_output_decl(pair: Pair<Rule>) -> OutputDecl {
    let mut inner = pair.into_inner();
    OutputDecl {
        name: inner.next().unwrap().as_str().to_string(),
        ty:   parse_ty(inner.next().unwrap()),
    }
}

fn parse_bullet_body(pair: Pair<Rule>) -> BulletBody {
    let children: Vec<Pair<Rule>> = pair.into_inner().collect();
    if children.is_empty() { panic!("bullet body is empty"); }

    match children[0].as_rule() {
        Rule::rust_block => {
            let code = children[0].clone().into_inner()
                .next().unwrap().as_str().to_string();
            BulletBody::Native { backend: Backend::Rust, code }
        }
        Rule::pipe => {
            BulletBody::Pipes(children.into_iter().map(parse_pipe).collect())
        }
        other => unreachable!("unexpected bullet_body child: {:?}", other),
    }
}

// ── Pipe ──────────────────────────────────────────────────────────────────────

fn parse_pipe(pair: Pair<Rule>) -> Pipe {
    let pipe_span = span_of(&pair);
    let mut inner = pair.into_inner();
    let inputs: Vec<String> = inner.next().unwrap().into_inner()
        .map(|p| p.as_str().to_string()).collect();
    let expr    = parse_pipe_val(inner.next().unwrap());
    let binding = inner.next().unwrap().into_inner()
        .next().unwrap().as_str().to_string();
    Pipe { inputs, expr, binding, span: pipe_span }
}

fn parse_pipe_val(pair: Pair<Rule>) -> Expr {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::tuple_expr => Expr::Tuple(inner.into_inner().map(parse_expr).collect()),
        Rule::expr       => parse_expr(inner),
        other => unreachable!("unexpected pipe_val: {:?}", other),
    }
}

fn parse_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let lhs       = parse_atom(inner.next().unwrap());
    match inner.next() {
        Some(op_pair) => {
            let op  = op_pair.as_str().trim().to_string();
            let rhs = parse_atom(inner.next().unwrap());
            Expr::BinOp(BinExpr { lhs, op, rhs })
        }
        None => Expr::Atom(lhs),
    }
}

fn parse_atom(pair: Pair<Rule>) -> Atom {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::call => {
            let mut ci = inner.into_inner();
            let name   = ci.next().unwrap().as_str().to_string();
            let args   = ci.map(parse_call_arg).collect();
            Atom::Call { name, args }
        }
        Rule::integer => Atom::Integer(inner.as_str().parse().unwrap()),
        Rule::ident   => Atom::Ident(inner.as_str().to_string()),
        other => unreachable!("unexpected atom: {:?}", other),
    }
}

fn parse_call_arg(pair: Pair<Rule>) -> CallArg {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::bullet_ref => CallArg::BulletRef(
            inner.into_inner().next().unwrap().as_str().to_string()
        ),
        Rule::integer => CallArg::Value(inner.as_str().to_string()),
        Rule::ident   => CallArg::Value(inner.as_str().to_string()),
        other => unreachable!("unexpected call_arg: {:?}", other),
    }
}

// ── Type ──────────────────────────────────────────────────────────────────────

fn parse_ty(pair: Pair<Rule>) -> BuType {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        // () — the unit type
        Rule::ty_unit  => BuType::Named("()".to_string()),
        Rule::ty_tuple => BuType::Tuple(inner.into_inner().map(parse_ty).collect()),
        Rule::ty_array => {
            let mut ai      = inner.into_inner();
            let elem        = parse_ty(ai.next().unwrap());
            let size: usize = ai.next().unwrap().as_str().parse().unwrap();
            BuType::Array(Box::new(elem), size)
        }
        Rule::ty_fn   => BuType::Named(inner.as_str().trim().to_string()),
        Rule::ty_atom => BuType::Named(inner.as_str().trim().to_string()),
        other => unreachable!("unexpected ty rule: {:?}", other),
    }
}
