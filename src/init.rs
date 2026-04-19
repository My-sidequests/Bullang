//! `bullang init` — project scaffolding.
//!
//! `bullang init my_project --depth N` creates a properly structured
//! Bullang folder tree. Depth maps directly to rank from the bottom up:
//!
//!   depth 1 → skirmish
//!   depth 2 → tactic → skirmish
//!   depth 3 → strategy → tactic → skirmish
//!   depth 4 → battle → strategy → tactic → skirmish
//!   depth 5 → theater → battle → strategy → tactic → skirmish
//!   depth 6 → war → theater → battle → strategy → tactic → skirmish

use std::path::{Path, PathBuf};
use std::fs;
use crate::ast::Rank;

// ── Rank from depth ───────────────────────────────────────────────────────────

pub fn rank_for_depth(depth: u8) -> Option<Rank> {
    match depth {
        1 => Some(Rank::Skirmish),
        2 => Some(Rank::Tactic),
        3 => Some(Rank::Strategy),
        4 => Some(Rank::Battle),
        5 => Some(Rank::Theater),
        6 => Some(Rank::War),
        _ => None,
    }
}

/// The chain of ranks from root down to skirmish for a given depth.
/// depth 3 → [Strategy, Tactic, Skirmish]
fn rank_chain(depth: u8) -> Vec<Rank> {
    let all = [
        Rank::War, Rank::Theater, Rank::Battle,
        Rank::Strategy, Rank::Tactic, Rank::Skirmish,
    ];
    let start = (all.len() as u8).saturating_sub(depth) as usize;
    all[start..].to_vec()
}

// ── Public entry point ────────────────────────────────────────────────────────

pub struct InitResult {
    pub root:          PathBuf,
    pub files_created: Vec<PathBuf>,
}

pub fn init(parent: &Path, name: &str, depth: u8, lang: Option<&str>, libs: &[String]) -> Result<InitResult, String> {
    let root = parent.join(name);

    if root.exists() {
        return Err(format!(
            "'{}' already exists. Choose a different name or remove the existing folder.",
            root.display()
        ));
    }

    let ranks  = rank_chain(depth);
    let total  = ranks.len();
    let mut files_created: Vec<PathBuf> = Vec::new();

    create_level(&root, &ranks, name, total, true, lang, libs, &mut files_created)?;

    Ok(InitResult { root, files_created })
}

// ── Recursive folder creator ──────────────────────────────────────────────────

fn create_level(
    dir:     &Path,
    ranks:   &[Rank],
    name:    &str,
    total:   usize,
    is_root: bool,
    lang:    Option<&str>,
    libs:    &[String],
    created: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let rank = &ranks[0];

    fs::create_dir_all(dir)
        .map_err(|e| format!("Could not create '{}': {}", dir.display(), e))?;

    // Write inventory.bu for this level.
    let mut inv_content = format!("#rank: {};\n", rank.name());
    if is_root {
        if let Some(l) = lang {
            inv_content.push('\n');
            inv_content.push_str(&format!("#lang: {};\n", l));
        }
        if !libs.is_empty() {
            inv_content.push('\n');
            for lib in libs {
                inv_content.push_str(&format!("#lib: {};\n", lib));
            }
        }
    }
    if rank == &Rank::Skirmish {
        inv_content.push_str("\nexample: hello;\n");
    }
    write_file(&dir.join("inventory.bu"), &inv_content, created)?;

    if ranks.len() == 1 {
        // Leaf (skirmish): write the example source file
        write_file(&dir.join("example.bu"), EXAMPLE_SOURCE, created)?;
    } else {
        // Recurse into one child folder
        let child_rank = &ranks[1];
        let child_name = format!("{}_{}", child_rank.name(), name);
        let child_dir  = dir.join(&child_name);
        create_level(&child_dir, &ranks[1..], name, total, false, None, &[], created)?;
    }

    // Write main.bu only at the root (not at intermediate levels)
    // and only when depth > 1 (skirmish-only projects don't have an entry point)
    if is_root && total > 1 {
        write_file(&dir.join("main.bu"), EXAMPLE_MAIN, created)?;
    }

    Ok(())
}

// ── File writer ───────────────────────────────────────────────────────────────

fn write_file(path: &Path, content: &str, created: &mut Vec<PathBuf>) -> Result<(), String> {
    fs::write(path, content)
        .map_err(|e| format!("Could not write '{}': {}", path.display(), e))?;
    created.push(path.to_path_buf());
    Ok(())
}

// ── Template content ──────────────────────────────────────────────────────────

const EXAMPLE_SOURCE: &str = r#"// Example skirmish function.
// Skirmish is the lowest rank — only raw expressions, no calls to other functions.
// All functions at this level are pure primitives.

let hello(x: i32) -> result: i32 {
    (x) : x + 1 -> {result};
}
"#;

const EXAMPLE_MAIN: &str = r#"// Entry point for this Bullang project.
// main.bu is not listed in inventory — it is always the executable entry point.
// Use @rust blocks here for I/O, CLI parsing, and orchestration.

let main() -> result: () {
    @rust
    println!("Hello from Bullang!");
    @end
}
"#;

// ── Display helper ────────────────────────────────────────────────────────────

pub fn print_tree(result: &InitResult) {
    println!("created: {}", result.root.display());
    println!();

    let root = &result.root;
    for path in &result.files_created {
        let rel   = path.strip_prefix(root).unwrap_or(path);
        // Depth = number of directory components (not counting filename itself)
        let depth = rel.components().count().saturating_sub(1);
        let indent = "  ".repeat(depth);
        let file_name = rel.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");

        // Show parent folder name before the first file at each new depth
        let label = match file_name {
            "inventory.bu" => format!("{} (inventory)", file_name),
            "main.bu"      => format!("{} (entry point)", file_name),
            _              => file_name.to_string(),
        };

        // Print folder header when we descend into a new directory
        if file_name == "inventory.bu" && depth > 0 {
            let folder = rel.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            let folder_indent = "  ".repeat(depth - 1);
            println!("  {}{}/", folder_indent, folder);
        }

        println!("  {}  {}", indent, label);
    }
}

// ── Blueprint ─────────────────────────────────────────────────────────────────
//
// A `blueprint.bu` describes the project structure with indented blocks.
// Each indented line is a folder name; the deepest indented lines can
// contain `filename: fn1, fn2;` entries (inventory entries).
//
// Example:
//
//   strategy_core:
//       tactic_engine:
//           skirmish_math:
//               math: add, subtract
//               utils: clamp
//           skirmish_io:
//               parsers: parse_int
//
// Rules:
//   - Indent unit is automatically detected (first indented line sets the unit).
//   - Folder lines end with `:`.
//   - Inventory lines match `ident: ident, ident;` — the `;` is optional.
//   - Blank lines and lines starting with `//` are ignored.
//   - The rank hierarchy is inferred from the depth of folders in the tree.
//   - `blueprint.bu` is copied to the project root unchanged.

#[derive(Debug)]
pub enum BlueprintNode {
    Folder { name: String, children: Vec<BlueprintNode> },
    Entry  { file: String, functions: Vec<String> },
}

/// Parse a blueprint.bu file into a list of top-level nodes.
pub fn parse_blueprint(source: &str) -> Result<Vec<BlueprintNode>, String> {
    let lines: Vec<&str> = source.lines().collect();
    let indent_unit = detect_indent_unit(&lines);
    parse_block(&lines, 0, 0, indent_unit).map(|(nodes, _)| nodes)
}

fn detect_indent_unit(lines: &[&str]) -> usize {
    for line in lines {
        if line.trim().is_empty() || line.trim_start().starts_with("//") { continue; }
        let spaces = line.len() - line.trim_start().len();
        if spaces > 0 { return spaces; }
    }
    4 // default
}

/// Recursively parse a block of lines at `base_indent` depth.
/// Returns (nodes, index of next unprocessed line).
fn parse_block(
    lines:       &[&str],
    start:       usize,
    base_indent: usize,
    unit:        usize,
) -> Result<(Vec<BlueprintNode>, usize), String> {
    let mut nodes = Vec::new();
    let mut i     = start;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Skip blank lines and comments
        if trimmed.is_empty() || trimmed.starts_with("//") { i += 1; continue; }

        let indent = line.len() - line.trim_start().len();

        // Less indented than our base → hand back to caller
        if indent < base_indent { break; }

        // More indented than expected → error
        if indent > base_indent {
            return Err(format!(
                "line {}: unexpected indentation (expected {} spaces, got {}): '{}'",
                i + 1, base_indent, indent, trimmed
            ));
        }

        // Folder line: ends with ':'
        if trimmed.ends_with(':') {
            let name = trimmed.trim_end_matches(':').trim().to_string();
            if name.is_empty() {
                return Err(format!("line {}: empty folder name", i + 1));
            }
            i += 1;
            // Collect children at base_indent + unit
            let (children, next) = parse_block(lines, i, base_indent + unit, unit)?;
            nodes.push(BlueprintNode::Folder { name, children });
            i = next;
        } else {
            // Inventory entry line: `filename: fn1, fn2` (optional trailing `;`)
            let entry = trimmed.trim_end_matches(';');
            if let Some(colon) = entry.find(':') {
                let file = entry[..colon].trim().to_string();
                let fns_str = entry[colon+1..].trim();
                let functions: Vec<String> = fns_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if file.is_empty() {
                    return Err(format!("line {}: empty file name in entry", i + 1));
                }
                nodes.push(BlueprintNode::Entry { file, functions });
            } else {
                return Err(format!(
                    "line {}: expected 'folder:' or 'filename: fn1, fn2' — got '{}'",
                    i + 1, trimmed
                ));
            }
            i += 1;
        }
    }

    Ok((nodes, i))
}

// ── Blueprint → folder tree ───────────────────────────────────────────────────

pub struct BlueprintResult {
    pub root:          PathBuf,
    pub files_created: Vec<PathBuf>,
}

/// Scaffold a project from a parsed blueprint.
///
/// `parent`     — where to create the project root folder
/// `name`       — the project root folder name
/// `nodes`      — parsed blueprint nodes (top-level folders/entries)
/// `lang`       — optional `#lang:` for root inventory
/// `blueprint_src` — raw source of blueprint.bu to copy to project root
pub fn init_from_blueprint(
    parent:        &Path,
    name:          &str,
    nodes:         &[BlueprintNode],
    lang:          Option<&str>,
    blueprint_src: &str,
) -> Result<BlueprintResult, String> {
    let root = parent.join(name);
    if root.exists() {
        return Err(format!(
            "'{}' already exists.", root.display()
        ));
    }
    fs::create_dir_all(&root)
        .map_err(|e| format!("Could not create '{}': {}", root.display(), e))?;

    let mut files_created: Vec<PathBuf> = Vec::new();

    // Infer root rank from the tree depth
    let max_depth = tree_depth(nodes);
    let root_rank = rank_for_depth(max_depth as u8)
        .ok_or_else(|| format!("Blueprint tree is too deep (max 6 levels)"))?;

    // Write root inventory.bu
    let mut root_inv = format!("#rank: {};\n", root_rank.name());
    if let Some(l) = lang {
        root_inv.push_str(&format!("\n#lang: {};\n", l));
    }
    // Root-level entries (non-folder nodes at top level)
    let root_entries: Vec<&BlueprintNode> = nodes.iter()
        .filter(|n| matches!(n, BlueprintNode::Entry { .. }))
        .collect();
    if !root_entries.is_empty() {
        root_inv.push('\n');
        for e in root_entries {
            if let BlueprintNode::Entry { file, functions } = e {
                root_inv.push_str(&format!("{}: {};\n", file, functions.join(", ")));
            }
        }
    }
    write_file(&root.join("inventory.bu"), &root_inv, &mut files_created)?;

    // Recurse into child folders
    let child_folders: Vec<&BlueprintNode> = nodes.iter()
        .filter(|n| matches!(n, BlueprintNode::Folder { .. }))
        .collect();
    for node in child_folders {
        if let BlueprintNode::Folder { name: folder_name, children } = node {
            emit_blueprint_folder(
                &root, folder_name, children, max_depth - 1, &mut files_created
            )?;
        }
    }

    // Copy blueprint.bu to project root
    write_file(&root.join("blueprint.bu"), blueprint_src, &mut files_created)?;

    Ok(BlueprintResult { root, files_created })
}

/// Recursively create a folder and its inventory.bu from blueprint nodes.
/// `depth_remaining` counts how many levels are left below this folder.
fn emit_blueprint_folder(
    parent:           &Path,
    name:             &str,
    children:         &[BlueprintNode],
    depth_remaining:  usize,
    created:          &mut Vec<PathBuf>,
) -> Result<(), String> {
    let dir = parent.join(name);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create '{}': {}", dir.display(), e))?;

    // This folder's rank = depth_remaining levels from skirmish
    let rank = rank_for_depth(depth_remaining as u8)
        .ok_or_else(|| format!("Blueprint nesting too deep at '{}'", name))?;

    let mut inv = format!("#rank: {};\n", rank.name());

    // Collect inventory entries from direct Entry children
    let entries: Vec<&BlueprintNode> = children.iter()
        .filter(|n| matches!(n, BlueprintNode::Entry { .. }))
        .collect();
    if !entries.is_empty() {
        inv.push('\n');
        for e in entries {
            if let BlueprintNode::Entry { file, functions } = e {
                inv.push_str(&format!("{}: {};\n", file, functions.join(", ")));
            }
        }
    }
    write_file(&dir.join("inventory.bu"), &inv, created)?;

    // Recurse into sub-folders
    for child in children {
        if let BlueprintNode::Folder { name: child_name, children: grandchildren } = child {
            emit_blueprint_folder(
                &dir, child_name, grandchildren, depth_remaining - 1, created
            )?;
        }
    }

    Ok(())
}

/// Max folder nesting depth in a blueprint tree (1 = no subfolders, just entries).
fn tree_depth(nodes: &[BlueprintNode]) -> usize {
    let mut max = 0usize;
    for node in nodes {
        if let BlueprintNode::Folder { children, .. } = node {
            let child_depth = 1 + tree_depth(children);
            if child_depth > max { max = child_depth; }
        }
    }
    // If max is 0 all nodes are entries → root is skirmish (depth 1)
    if max == 0 { 1 } else { max }
}

// ── Blueprint display ─────────────────────────────────────────────────────────

pub fn print_blueprint_tree(result: &BlueprintResult) {
    println!("created: {}", result.root.display());
    println!();
    let root = &result.root;
    for path in &result.files_created {
        let rel   = path.strip_prefix(root).unwrap_or(path);
        let depth = rel.components().count().saturating_sub(1);
        let indent = "  ".repeat(depth);
        let file_name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        if file_name == "inventory.bu" && depth > 0 {
            let folder = rel.parent()
                .and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("?");
            println!("  {}{}/", "  ".repeat(depth - 1), folder);
        }
        let label = match file_name {
            "inventory.bu" => format!("{} (inventory)", file_name),
            "blueprint.bu" => format!("{} (blueprint — copied)", file_name),
            _ => file_name.to_string(),
        };
        println!("  {}  {}", indent, label);
    }
}
