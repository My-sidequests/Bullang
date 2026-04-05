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

    let rule = if is_inventory {
        Rule::inventory_file
    } else {
        Rule::skirmish_file
    };

    let mut pairs = BulParser::parse(rule, source)?;
    let file_pair = pairs.next().unwrap();

    if is_inventory {
        Ok(BuFile::Inventory(parse_inventory(file_pair)))
    } else {
        Ok(BuFile::Skirmish(parse_skirmish(file_pair)))
    }
}

// ── File parsers ──────────────────────────────────────────────────────────────

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
            Rule::bullet => {
                bullets.push(parse_bullet(inner));
            }
            Rule::EOI => {}
            _ => {}
        }
    }

    let mut bullets: Vec<Bullet> = bullets.into_iter().map(|mut b| {
        b.exported = exports.contains(&b.name);
        b
    }).collect();

    SkirmishFile {
        rank:     rank.expect("missing #rank"),
        category: category.expect("missing #category"),
        imports,
        exports,
        bullets,
    }
}

fn parse_inventory(pair: Pair<Rule>) -> InventoryFile {
    let mut rank    = None;
    let mut exports = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::dir_rank => {
                rank = Rank::from_str(
                    inner.into_inner().next().unwrap().as_str()
                );
            }
            Rule::dir_export => {
                exports = inner.into_inner()
                    .map(|p| p.as_str().to_string())
                    .collect();
            }
            Rule::EOI => {}
            _ => {}
        }
    }

    InventoryFile {
        rank:    rank.expect("missing #rank"),
        exports,
    }
}

// ── Bullet parser ─────────────────────────────────────────────────────────────

fn parse_bullet(pair: Pair<Rule>) -> Bullet {
    let mut inner = pair.into_inner();

    let name       = inner.next().unwrap().as_str().to_string();
    let param_list = inner.next().unwrap();
    let params     = parse_param_list(param_list);
    let output     = parse_output_decl(inner.next().unwrap());
    let pipes      = inner.map(parse_pipe).collect();

    Bullet { name, params, output, pipes, exported: false }
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

// ── Type parser ───────────────────────────────────────────────────────────────

fn parse_ty(pair: Pair<Rule>) -> BuType {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::ty_generic => {
            // Collect the full raw string — it's a valid Rust type already
            BuType::Generic(inner.as_str().to_string())
        }
        Rule::ty_tuple => {
            let types = inner.into_inner().map(parse_ty).collect();
            BuType::Tuple(types)
        }
        Rule::ty_array => {
            let mut ai  = inner.into_inner();
            let elem_ty = parse_ty(ai.next().unwrap());
            let size    = ai.next().unwrap().as_str().parse().unwrap();
            BuType::Array(Box::new(elem_ty), size)
        }
        Rule::ty_primitive => {
            BuType::Generic(inner.as_str().to_string())
        }
        _ => unreachable!(),
    }
}

// ── Pipe parser ───────────────────────────────────────────────────────────────

fn parse_pipe(pair: Pair<Rule>) -> Pipe {
    let mut inner = pair.into_inner();

    let input_list = inner.next().unwrap();
    let inputs: Vec<String> = input_list.into_inner()
        .map(|p| p.as_str().to_string())
        .collect();

    let pipe_val = inner.next().unwrap();
    let expr     = parse_pipe_val(pipe_val);

    let binding_pair = inner.next().unwrap();
    let binding = binding_pair.into_inner().next().unwrap().as_str().to_string();

    Pipe { inputs, expr, binding }
}

fn parse_pipe_val(pair: Pair<Rule>) -> Expr {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::tuple_expr => {
            let exprs = inner.into_inner().map(parse_expr).collect();
            Expr::Tuple(exprs)
        }
        Rule::expr => parse_expr(inner),
        _          => unreachable!(),
    }
}

fn parse_expr(pair: Pair<Rule>) -> Expr {
    let mut inner = pair.into_inner();
    let lhs_atom  = parse_atom(inner.next().unwrap());

    if let Some(op_pair) = inner.next() {
        let op       = op_pair.as_str().trim().to_string();
        let rhs_atom = parse_atom(inner.next().unwrap());
        Expr::BinOp(BinExpr { lhs: lhs_atom, op, rhs: rhs_atom })
    } else {
        Expr::Atom(lhs_atom)
    }
}

fn parse_atom(pair: Pair<Rule>) -> Atom {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::call => {
            let mut ci   = inner.into_inner();
            let name     = ci.next().unwrap().as_str().to_string();
            let args     = ci.map(parse_call_arg).collect();
            Atom::Call { name, args }
        }
        Rule::integer => Atom::Integer(inner.as_str().parse().unwrap()),
        Rule::ident   => Atom::Ident(inner.as_str().to_string()),
        _             => unreachable!(),
    }
}

fn parse_call_arg(pair: Pair<Rule>) -> CallArg {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::bullet_ref => {
            let name = inner.into_inner().next().unwrap().as_str().to_string();
            CallArg::BulletRef(name)
        }
        Rule::ident => CallArg::Value(inner.as_str().to_string()),
        _           => unreachable!(),
    }
}
