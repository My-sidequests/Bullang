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
    write_file(&dir.join("inventory.bu"), &inv_content, created)?;

    if ranks.len() > 1 {
        // Recurse into one child folder
        let child_rank = &ranks[1];
        let child_name = format!("{}_{}", child_rank.name(), name);
        let child_dir  = dir.join(&child_name);
        create_level(&child_dir, &ranks[1..], name, total, false, None, &[], created)?;
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

// ── Bullang README (generated into every blueprint project) ──────────────────

const BULLANG_README: &str = "# Bullang

Bullang is a structured functional language that transpiles to Rust, Python, C, C++, and Go.
Every project is a hierarchy of folders. Each folder has a rank. Source files live only at
the leaf rank (skirmish). This file was generated by `bullang init --blueprint`.

---

## Hierarchy

Ranks from highest to lowest:

```
war > theater > battle > strategy > tactic > skirmish
```

Each folder must contain an `inventory.bu` declaring its rank.  \
Skirmish folders hold source files. All other ranks hold sub-folders.

Limits per level: 5 sub-folders · 5 source files · 5 functions per file · 5 bullets per function.

---

## Writing a function

Every source file contains only function declarations — no imports, no metadata.

```
let add(a: i32, b: i32) -> result: i32 {
    (a, b) : a + b -> {result};
}
```

`let name(params) -> output_name: ReturnType { body }`

A bullet (pipe statement): `(inputs) : expression -> {binding};`

Rules:
- Every binding must be consumed by a later bullet, except the final output.
- The last bullet must bind to the declared output name.
- No binding may be assigned twice.
- Maximum 5 bullets per function.

---

## Consuming arguments across bullets

Bullets pass values forward by name. Each bullet takes inputs from the parameter
list or from names bound by earlier bullets in the same function.

```
let scale_and_add(x: i32, y: i32, factor: i32) -> result: i32 {
    (x, factor) : x * factor -> {scaled};
    (scaled, y) : scaled + y -> {result};
}
```

`scaled` is produced by bullet 1 and consumed by bullet 2.
`result` matches the declared output, so the function returns it.

---

## Inventory files

Every folder must have `inventory.bu`:

```
#rank: skirmish;
#lang: rs;           ← optional: default convert target
#lib: stdio.h;       ← optional: C/C++ header include (repeatable)

math    : add, subtract, multiply;
helpers : clamp, abs_val;
```

Rules:
- Every .bu source file in the folder must be listed.
- Every function in a file must be listed next to its filename.
- `main.bu` and `blueprint.bu` are reserved — never list them.

---

## Native escape blocks

For logic Bullang pipes cannot express:

```
let sum_vec(values: Vec[i32]) -> result: i32 {
    @rust
    values.iter().sum()
    @end
}
```

Backends: `@rust`  `@python`  `@c`  `@cpp`  `@go`

---

## Builtin standard library

```
let upper(s: String) -> result: String {
    builtin::to_upper
}
```

Universal builtins (work in all 5 backends):

| Category | Builtins |
|----------|----------|
| Math | `abs` `pow` `powf` `sqrt` `clamp` |
| String | `to_upper` `to_lower` `trim` `starts_with` `ends_with` `replace_str` `to_string` `parse_i64` |

Run `bullang stdlib --list` for full signatures and parameter counts.

---

## Types

| Bullang | Description |
|---------|-------------|
| `Vec[T]` | Dynamic array |
| `Option[T]` | Nullable value |
| `Tuple[T, U]` | Fixed-size tuple |
| `Fn[T -> U]` | Function reference |
| `i32`, `i64`, `f64`... | Numeric primitives |
| `bool`, `char`, `String` | Other primitives |

---

## Commands

```
bullang check                validate from anywhere in the tree
bullang convert my_project   transpile (reads #lang, defaults to Rust)
bullang convert file.bu      transpile a single file to stdout
bullang init --help          all init options
bullang stdlib --list        available builtins
bullang lsp                  start the language server
bullang editor-setup         write LSP config for Neovim / Helix / Emacs
bullang update               pull and rebuild from the repository
```
";

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

    // Copy blueprint.bu and README.md to the project root
    write_file(&root.join("blueprint.bu"), blueprint_src, &mut files_created)?;
    write_file(&root.join("README.md"), BULLANG_README, &mut files_created)?;

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
