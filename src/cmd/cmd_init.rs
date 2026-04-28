//! `bullang init` — project scaffolding command.

use std::path::PathBuf;
use crate::init;
use crate::utils::current_dir;
use crate::readme::write_project_readme;

pub fn cmd_init(
    name:      String,
    depth:     u8,
    blueprint: Option<PathBuf>,
    lang:      Option<String>,
    libs:      Vec<String>,
    path:      Option<PathBuf>,
) {
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
                write_project_readme(&parent.join(&name));
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
            write_project_readme(&parent.join(&name));
        }
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    }
}
