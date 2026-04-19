//! Tree-walk build pass — rank-agnostic, any rank as root.
//! Dispatches to Rust or Python codegen based on the target backend.

use std::path::{Path, PathBuf};
use std::fs;

use crate::ast::{BuFile, Rank, Backend};
use crate::codegen;
use crate::codegen_c;
use crate::codegen_cpp;
use crate::codegen_go;
use crate::codegen_python;
use crate::parser;
use crate::validator::{
    ValidationError, collect_bu_files, collect_subdirs,
    read_inventory, main_bu_path,
};

pub struct BuildResult {
    pub errors:        Vec<ValidationError>,
    pub files_written: usize,
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn build(root: &Path, out_dir: &Path, crate_name: &str, backend: &Backend) -> BuildResult {
    let mut errors        = Vec::new();
    let mut files_written = 0;

    // Python: package dir named after the crate so imports work.
    // Rust: conventional src/ layout.
    let src_out = match backend {
        Backend::Python => out_dir.join(crate_name),
        Backend::Go | Backend::C | Backend::Cpp => out_dir.to_path_buf(),
        _ => out_dir.join("src"),
    };
    fs::create_dir_all(&src_out).expect("could not create out/src");

    let has_main = tree_has_main(root);

    let (child_modules, _) = emit_folder(
        root, &src_out, backend, crate_name, has_main, &mut errors, &mut files_written,
    );

    match backend {
        Backend::Rust => {
            write_file(
                &src_out.join("lib.rs"),
                &codegen::emit_lib_rs(&child_modules),
                &mut files_written,
            );
            let cargo = if has_main {
                codegen::emit_cargo_toml_with_main(crate_name)
            } else {
                codegen::emit_cargo_toml(crate_name)
            };
            write_file(&out_dir.join("Cargo.toml"), &cargo, &mut files_written);
        }
        Backend::Python => {
            write_file(
                &src_out.join("__init__.py"),
                &codegen_python::emit_init_py(&child_modules),
                &mut files_written,
            );
        }
        Backend::C => {
            let header_name  = format!("{}.h", crate_name);
            let all_sources  = collect_all_sources(root);
            let src_refs: Vec<(String, &crate::ast::SourceFile)> =
                all_sources.iter().map(|(n, sf)| (n.clone(), sf)).collect();
            let libs = collect_all_libs(root);
            let header = codegen_c::emit_header_c(crate_name, &src_refs, &libs);
            write_file(&out_dir.join(&header_name), &header, &mut files_written);

            let mut all_c: Vec<String> = child_modules.iter()
                .map(|m| format!("{}.c", m)).collect();
            if has_main { all_c.push("main.c".to_string()); }
            let makefile = codegen_c::emit_makefile(crate_name, &all_c, has_main);
            write_file(&out_dir.join("Makefile"), &makefile, &mut files_written);
        }
        Backend::Cpp => {
            let header_name  = format!("{}.hpp", crate_name);
            let all_sources  = collect_all_sources(root);
            let src_refs: Vec<(String, &crate::ast::SourceFile)> =
                all_sources.iter().map(|(n, sf)| (n.clone(), sf)).collect();
            let libs = collect_all_libs(root);
            let header = codegen_cpp::emit_header_cpp(crate_name, &src_refs, crate_name, &libs);
            write_file(&out_dir.join(&header_name), &header, &mut files_written);

            let mut all_cpp: Vec<String> = child_modules.iter()
                .map(|m| format!("{}.cpp", m)).collect();
            if has_main { all_cpp.push("main.cpp".to_string()); }
            let makefile = codegen_cpp::emit_makefile_cpp(crate_name, &all_cpp, has_main);
            write_file(&out_dir.join("Makefile"), &makefile, &mut files_written);
        }
        Backend::Go => {
            write_file(&out_dir.join("go.mod"), &codegen_go::emit_go_mod(crate_name), &mut files_written);
        }
        Backend::Unknown(_) => {}
    }

    // ── blueprint.md ─────────────────────────────────────────────────────────
    // If the project root contains a blueprint.bu, copy it as blueprint.md
    // into the output so the architecture is documented alongside the code.
    let bp_src = root.join("blueprint.bu");
    if bp_src.exists() {
        if let Ok(bp_content) = fs::read_to_string(&bp_src) {
            let out_path = match backend {
                Backend::Python => out_dir.join(crate_name).join("blueprint.md"),
                Backend::C | Backend::Cpp | Backend::Go => out_dir.join("blueprint.md"),
                _ => src_out.join("blueprint.md"),
            };
            write_file(&out_path, &bp_content, &mut files_written);
        }
    }

    BuildResult { errors, files_written }
}

// ── Recursive folder emitter ──────────────────────────────────────────────────

fn emit_folder(
    src_dir:    &Path,
    out_dir:    &Path,
    backend:    &Backend,
    crate_name: &str,
    has_main:   bool,
    errors:     &mut Vec<ValidationError>,
    written:    &mut usize,
) -> (Vec<String>, Vec<String>) {
    let inv = match read_inventory(src_dir) {
        Ok(i)  => i,
        Err(_) => return (vec![], vec![]),
    };

    let mut child_modules: Vec<String> = Vec::new();
    let mut all_fns:       Vec<String> = Vec::new();

    // War: only sub-folders (+ optional main.bu)
    if inv.rank == Rank::War {
        for subdir in collect_subdirs(src_dir) {
            let name      = dir_name(&subdir);
            let child_out = out_dir.join(&name);
            fs::create_dir_all(&child_out).ok();
            let (gc, fns) = emit_folder(&subdir, &child_out, backend, crate_name, has_main, errors, written);
            emit_mod_file(&child_out, &gc, backend, written);
            merge(&fns, &mut all_fns);
            child_modules.push(name);
        }
        if let Some(mp) = main_bu_path(src_dir) {
            emit_main_file(&mp, out_dir, backend, crate_name, errors, written);
        }
        return (child_modules, all_fns);
    }

    // Sub-folders first (bottom-up)
    if inv.rank.has_sub_folders() {
        for subdir in collect_subdirs(src_dir) {
            let name = dir_name(&subdir);
            // C/C++: emit everything flat into the same output dir (no subdirectories)
            let child_out = match backend {
                Backend::C | Backend::Cpp | Backend::Go => out_dir.to_path_buf(),
                _ => {
                    let co = out_dir.join(&name);
                    fs::create_dir_all(&co).ok();
                    co
                }
            };
            let (gc, fns) = emit_folder(&subdir, &child_out, backend, crate_name, has_main, errors, written);
            if !matches!(backend, Backend::C | Backend::Cpp | Backend::Go) {
                emit_mod_file(&child_out, &gc, backend, written);
                child_modules.push(name);
            } else {
                // For C/C++, propagate the module names up so Makefile lists them
                child_modules.extend(gc);
            }
            merge(&fns, &mut all_fns);
        }
    }

    // Source files in inventory order
    if inv.rank.has_own_files() {
        for entry in &inv.entries {
            let bu_path = src_dir.join(format!("{}.bu", entry.file));
            let source  = match fs::read_to_string(&bu_path) {
                Ok(s)  => s,
                Err(e) => { errors.push(io_err(&bu_path, e)); continue; }
            };
            let sf = match parser::parse_file(&source, false) {
                Ok(BuFile::Source(s))    => s,
                Ok(BuFile::Inventory(_)) => continue,
                Err(e) => { errors.push(parse_err(&bu_path, e)); continue; }
            };

            merge(&entry.functions, &mut all_fns);

            let ext      = backend.ext();
            let out_path = out_dir.join(format!("{}.{}", entry.file, ext));
            let header_name = format!("{}.h", crate_name);
            let hpp_name    = format!("{}.hpp", crate_name);
            // Go: if has_main, all files must be in "main" package
            let go_pkg = if has_main && matches!(backend, Backend::Go) {
                "main".to_string()
            } else {
                crate_name.to_string()
            };
            let content  = match backend {
                Backend::Rust        => codegen::emit_source(&sf),
                Backend::Python      => codegen_python::emit_source_py(&sf),
                Backend::C           => codegen_c::emit_source_c(&sf, &header_name),
                Backend::Cpp         => codegen_cpp::emit_source_cpp(&sf, &hpp_name),
                Backend::Go          => codegen_go::emit_source_go(&sf, &go_pkg),
                Backend::Unknown(_)  => continue,
            };
            write_file(&out_path, &content, written);
            child_modules.push(entry.file.clone());
        }
    }

    // main.bu at non-skirmish levels
    if inv.rank != Rank::Skirmish {
        if let Some(mp) = main_bu_path(src_dir) {
            emit_main_file(&mp, out_dir, backend, crate_name, errors, written);
        }
    }

    (child_modules, all_fns)
}

// ── Module file emitter ───────────────────────────────────────────────────────

fn emit_mod_file(dir: &Path, child_modules: &[String], backend: &Backend, written: &mut usize) {
    match backend {
        Backend::Rust => {
            write_file(&dir.join("mod.rs"), &codegen::emit_mod_rs(child_modules), written);
        }
        Backend::Python => {
            write_file(
                &dir.join("__init__.py"),
                &codegen_python::emit_init_py(child_modules),
                written,
            );
        }
        Backend::C | Backend::Cpp | Backend::Go | Backend::Unknown(_) => {}
    }
}

// ── main.bu emitter ───────────────────────────────────────────────────────────

fn emit_main_file(
    main_path:  &Path,
    out_dir:    &Path,
    backend:    &Backend,
    crate_name: &str,
    errors:     &mut Vec<ValidationError>,
    written:    &mut usize,
) {
    let source = match fs::read_to_string(main_path) {
        Ok(s)  => s,
        Err(e) => { errors.push(io_err(main_path, e)); return; }
    };
    let sf = match parser::parse_file(&source, false) {
        Ok(BuFile::Source(s)) => s,
        Ok(BuFile::Inventory(_)) => return,
        Err(e) => { errors.push(parse_err(main_path, e)); return; }
    };

    let header_name = format!("{}.h", crate_name);
    let hpp_name    = format!("{}.hpp", crate_name);
    match backend {
        Backend::Rust => {
            write_file(
                &out_dir.join("main.rs"),
                &codegen::emit_main(&sf, crate_name),
                written,
            );
        }
        Backend::Python => {
            write_file(
                &out_dir.join("__main__.py"),
                &codegen_python::emit_main_py(&sf, crate_name),
                written,
            );
        }
        Backend::C => {
            write_file(
                &out_dir.join("main.c"),
                &codegen_c::emit_main_c(&sf, &header_name),
                written,
            );
        }
        Backend::Cpp => {
            write_file(
                &out_dir.join("main.cpp"),
                &codegen_cpp::emit_main_cpp(&sf, &hpp_name, crate_name),
                written,
            );
        }
        Backend::Go => {
            write_file(
                &out_dir.join("main.go"),
                &codegen_go::emit_main_go(&sf, crate_name),
                written,
            );
        }
        Backend::Unknown(_) => {}
    }
}

// ── Backend mismatch validation ───────────────────────────────────────────────

/// Validate that all escape blocks in the tree match the target backend.
/// Returns errors for any mismatch found.
pub fn validate_backend_compatibility(
    root:    &Path,
    backend: &Backend,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    check_folder_backend(root, backend, &mut errors);
    errors
}

fn check_folder_backend(dir: &Path, backend: &Backend, errors: &mut Vec<ValidationError>) {
    for bu in collect_bu_files(dir) {
        check_file_backend(&bu, backend, errors);
    }
    if let Some(mp) = main_bu_path(dir) {
        check_file_backend(&mp, backend, errors);
    }
    for subdir in collect_subdirs(dir) {
        check_folder_backend(&subdir, backend, errors);
    }
}

fn check_file_backend(path: &Path, backend: &Backend, errors: &mut Vec<ValidationError>) {
    let source = match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(_) => return,
    };
    let sf = match parser::parse_file(&source, false) {
        Ok(BuFile::Source(s)) => s,
        _                     => return,
    };

    let path_str = path.display().to_string();
    for func in &sf.bullets {
        if let crate::ast::BulletBody::Native { backend: block_backend, .. } = &func.body {
            if block_backend != backend {
                let expected_kw = backend.escape_keyword();
                let actual_kw   = block_backend.escape_keyword();
                if let Backend::Unknown(_) = block_backend {
                    continue; // already caught by validator
                }
                // @c blocks are also valid in C++ projects
                let compat = match (block_backend, backend) {
                    (Backend::C, Backend::Cpp) => true,
                    (a, b) => a == b,
                };
                if !compat {
                    errors.push(ValidationError {
                        file:    path_str.clone(),
                        line:    func.span.line,
                        col:     func.span.col,
                        message: format!(
                            "Function '{}': '@{}' block cannot be used when building \
                             for '{}' backend. Use '@{}' instead.",
                            func.name, actual_kw, backend.name(), expected_kw
                        ),
                    });
                }
            }
        }
    }
}

// ── Library collector (for header #include directives) ───────────────────────

/// Walk the entire source tree and collect all unique #lib declarations.
/// Libs from all inventories are merged — deeper inventories can add to
/// the global set. Order is deterministic (tree walk order, deduped).
fn collect_all_libs(dir: &Path) -> Vec<String> {
    let mut libs: Vec<String> = Vec::new();
    if let Ok(inv) = read_inventory(dir) {
        for lib in &inv.libs {
            if !libs.contains(lib) {
                libs.push(lib.clone());
            }
        }
    }
    for subdir in collect_subdirs(dir) {
        for lib in collect_all_libs(&subdir) {
            if !libs.contains(&lib) {
                libs.push(lib);
            }
        }
    }
    libs
}

// ── Source file collector (for header generation) ────────────────────────────

/// Walk the entire source tree and collect (stem_name, SourceFile) for every
/// .bu source file. Used by C/C++ header generation to produce forward decls.
fn collect_all_sources(dir: &Path) -> Vec<(String, crate::ast::SourceFile)> {
    let mut result = Vec::new();
    let inv = match read_inventory(dir) {
        Ok(i) => i, Err(_) => return result,
    };
    for entry in &inv.entries {
        let bu_path = dir.join(format!("{}.bu", entry.file));
        if let Ok(source) = std::fs::read_to_string(&bu_path) {
            if let Ok(crate::ast::BuFile::Source(sf)) = parser::parse_file(&source, false) {
                result.push((entry.file.clone(), sf));
            }
        }
    }
    for subdir in collect_subdirs(dir) {
        result.extend(collect_all_sources(&subdir));
    }
    result
}

// ── Tree scan ─────────────────────────────────────────────────────────────────

fn tree_has_main(dir: &Path) -> bool {
    if main_bu_path(dir).is_some() { return true; }
    for subdir in collect_subdirs(dir) {
        if tree_has_main(&subdir) { return true; }
    }
    false
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn write_file(path: &Path, content: &str, written: &mut usize) {
    if let Some(p) = path.parent() { fs::create_dir_all(p).ok(); }
    if fs::write(path, content).is_ok() { *written += 1; }
}

fn dir_name(path: &Path) -> String {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string()
}

fn merge(src: &[String], dst: &mut Vec<String>) {
    for name in src { if !dst.contains(name) { dst.push(name.clone()); } }
}

fn io_err(path: &Path, e: std::io::Error) -> ValidationError {
    ValidationError { file: path.display().to_string(), line: 0, col: 0,
        message: format!("Could not read: {}", e) }
}

fn parse_err(path: &Path, e: Box<dyn std::error::Error>) -> ValidationError {
    ValidationError { file: path.display().to_string(), line: 0, col: 0,
        message: format!("Parse error: {}", e) }
}
