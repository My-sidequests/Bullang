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
        let mut pairs = BulParser::parse(Rule::skirmish_file, source)?;
        Ok(BuFile::Skirmish(parse_skirmish(pairs.next().unwrap())))
    }
}

// ── Span extraction ───────────────────────────────────────────────────────────

fn span_of(pair: &Pair<Rule>) -> Span {
    let (line, col) = pair.as_span().start_pos().line_col();
    Span::new(line, col)
}

// ── File-level parsers ────────────────────────────────────────────────────────

fn parse_skirmish(pair: Pair<Rule>) -> SkirmishFile {
    let mut rank     = None;
    let mut category = None;
    let mut imports  = Vec::new();
    let mut exports  = Vec::new();
    let mut bullets  = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::dir_rank => {
                rank = Rank::from_str(
                    inner.into_inner().next().unwrap().as_str()
                );
            }
            Rule::dir_category => {
                category = Category::from_str(
                    inner.into_inner().next().unwrap().as_str()
                );
            }
            Rule::dir_import => {
                imports.push(
                    inner.into_inner().next().unwrap().as_str().to_string()
                );
            }
            Rule::dir_export => {
                exports = inner.into_inner()
                    .map(|p| p.as_str().to_string())
                    .collect();
            }
            Rule::bullet => bullets.push(parse_bullet(inner)),
            Rule::EOI    => {}
            _            => {}
        }
    }

    let bullets = bullets
        .into_iter()
        .map(|mut b| { b.exported = exports.contains(&b.name); b })
        .collect();

    SkirmishFile {
        rank:     rank.expect("missing #rank directive"),
        category: category.expect("missing #category directive"),
        imports,
        exports,
        bullets,
    }
}

fn parse_inventory(pair: Pair<Rule>) -> InventoryFile {
    let mut rank = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::dir_rank => {
                rank = Rank::from_str(
                    inner.into_inner().next().unwrap().as_str()
                );
            }
            Rule::EOI => {}
            _         => {}
        }
    }

    InventoryFile {
        rank:    rank.expect("missing #rank directive"),
        exports: Vec::new(),
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

    Bullet { name, params, output, body, exported: false, span: bullet_span }
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

    if children.is_empty() {
        panic!("bullet body is empty");
    }

    match children[0].as_rule() {
        Rule::rust_block => {
            let code = children[0]
                .clone()
                .into_inner()
                .next()
                .unwrap()
                .as_str()
                .to_string();
            BulletBody::Native { backend: Backend::Rust, code }
        }
        Rule::pipe => {
            let pipes = children.into_iter().map(parse_pipe).collect();
            BulletBody::Pipes(pipes)
        }
        other => unreachable!("unexpected bullet_body child: {:?}", other),
    }
}

// ── Pipe ──────────────────────────────────────────────────────────────────────

fn parse_pipe(pair: Pair<Rule>) -> Pipe {
    let pipe_span = span_of(&pair);
    let mut inner = pair.into_inner();

    let inputs: Vec<String> = inner
        .next().unwrap()
        .into_inner()
        .map(|p| p.as_str().to_string())
        .collect();

    let expr    = parse_pipe_val(inner.next().unwrap());
    let binding = inner
        .next().unwrap()
        .into_inner()
        .next().unwrap()
        .as_str()
        .to_string();

    Pipe { inputs, expr, binding, span: pipe_span }
}

fn parse_pipe_val(pair: Pair<Rule>) -> Expr {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::tuple_expr => {
            let exprs = inner.into_inner().map(parse_expr).collect();
            Expr::Tuple(exprs)
        }
        Rule::expr => parse_expr(inner),
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
        Rule::bullet_ref => {
            let name = inner.into_inner().next().unwrap().as_str().to_string();
            CallArg::BulletRef(name)
        }
        Rule::integer => CallArg::Value(inner.as_str().to_string()),
        Rule::ident   => CallArg::Value(inner.as_str().to_string()),
        other => unreachable!("unexpected call_arg: {:?}", other),
    }
}

// ── Type ──────────────────────────────────────────────────────────────────────

fn parse_ty(pair: Pair<Rule>) -> BuType {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::ty_tuple => {
            let types = inner.into_inner().map(parse_ty).collect();
            BuType::Tuple(types)
        }
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
