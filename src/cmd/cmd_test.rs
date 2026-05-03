//! `bullang test` — discovers and runs #test-annotated functions.
//!
//! Test discovery: walks the project tree, finds every bullet annotated with
//! `#test`, generates a temporary test harness in the target language, compiles
//! and runs it, and reports pass/fail per test.
//!
//! For Rust the harness uses cargo test. For C/C++ it emits a standalone
//! main.c that calls each test function. For Python it emits a __test__.py
//! using the standard `unittest` module. For Go it emits *_test.go files.

use std::path::{Path, PathBuf};
use std::fs;

use crate::ast::*;
use crate::parser;
use crate::validator::{self, read_inventory, collect_bu_files, collect_subdirs};
use crate::utils::{current_dir, find_root_from};

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn cmd_test(folder: Option<PathBuf>, ext: String) {
    let source_dir = match folder {
        Some(ref p) => p.canonicalize().unwrap_or_else(|_| p.clone()),
        None        => current_dir(),
    };
    let root = find_root_from(&source_dir);
    let inv  = validator::read_inventory(&root)
        .expect("could not read root inventory");

    let backend = inv.lang
        .as_ref()
        .map(|l| Backend::from_ext(l.ext()))
        .flatten()
        .or_else(|| Backend::from_ext(&ext))
        .unwrap_or(Backend::Rust);

    let tests = collect_tests(&root);

    if tests.is_empty() {
        println!("No #test functions found.");
        println!("Annotate a function with #test to register it:");
        println!();
        println!("  #test");
        println!("  let my_test() -> result: bool {{");
        println!("      ...");
        println!("  }}");
        return;
    }

    println!("bullang test");
    println!("  root    : {}", root.display());
    println!("  backend : {}", backend.name());
    println!("  found   : {} test(s)", tests.len());
    println!();

    let tmp = std::env::temp_dir().join("bullang_test");
    fs::create_dir_all(&tmp).expect("could not create temp dir");

    match backend {
        Backend::Rust   => run_rust_tests(&root, &tests, &tmp),
        Backend::C      => run_c_tests(&tests, &tmp),
        Backend::Cpp    => run_cpp_tests(&tests, &tmp),
        Backend::Python => run_python_tests(&tests, &tmp),
        Backend::Go     => run_go_tests(&tests, &tmp),
        Backend::Unknown(ref kw) => {
            eprintln!("error: unknown backend '{}'", kw);
            std::process::exit(1);
        }
    }
}

// ── Test discovery ────────────────────────────────────────────────────────────

pub struct TestFn {
    pub name:   String,
    pub module: String,
}

fn collect_tests(root: &Path) -> Vec<TestFn> {
    let mut out = Vec::new();
    collect_tests_in(root, &mut out);
    out
}

fn collect_tests_in(dir: &Path, out: &mut Vec<TestFn>) {
    let inv = match read_inventory(dir) {
        Ok(i)  => i,
        Err(_) => return,
    };
    for subdir in collect_subdirs(dir) {
        collect_tests_in(&subdir, out);
    }
    for bu_path in collect_bu_files(dir) {
        let src = match fs::read_to_string(&bu_path) {
            Ok(s)  => s,
            Err(_) => continue,
        };
        let sf = match parser::parse_file(&src, false) {
            Ok(BuFile::Source(s)) => s,
            _                     => continue,
        };
        let file_stem = bu_path.file_stem()
            .and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let module = inv.entries.iter()
            .find(|e| e.file == file_stem)
            .map(|_| file_stem.clone())
            .unwrap_or_else(|| file_stem.clone());

        for func in sf.bullets.iter().filter(|b| b.is_test) {
            out.push(TestFn {
                name:   func.name.clone(),
                module: module.clone(),
            });
        }
    }
}

// ── Rust test runner ──────────────────────────────────────────────────────────

fn run_rust_tests(root: &Path, tests: &[TestFn], tmp: &Path) {
    // Convert the project then append #[test] wrappers to lib.rs
    let crate_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("test");
    let out_dir    = tmp.join(crate_name);

    // Run convert first to produce the crate
    let status = std::process::Command::new("bullang")
        .args(["convert", &root.display().to_string(),
               "-n", &format!("{}_test", crate_name),
               "--out", &out_dir.display().to_string()])
        .status();

    if let Ok(s) = status {
        if !s.success() {
            eprintln!("error: convert step failed — fix errors before running tests");
            std::process::exit(1);
        }
    }

    // Append #[cfg(test)] wrappers
    let lib_rs = out_dir.join("src").join("lib.rs");
    let mut extra = String::new();
    extra.push_str("\n#[cfg(test)]\nmod tests {\n    use super::*;\n");
    for t in tests {
        extra.push_str(&format!(
            "    #[test]\n    fn {}() {{\n        assert!(super::{}());\n    }}\n",
            t.name, t.name
        ));
    }
    extra.push_str("}\n");

    if let Ok(mut content) = fs::read_to_string(&lib_rs) {
        content.push_str(&extra);
        let _ = fs::write(&lib_rs, content);
    }

    println!("running tests via cargo...");
    let _ = std::process::Command::new("cargo")
        .args(["test"])
        .current_dir(&out_dir)
        .status();
}

// ── C test runner ─────────────────────────────────────────────────────────────

fn run_c_tests(tests: &[TestFn], tmp: &Path) {
    // Emit a minimal test main.c that calls each #test function and checks its bool return.
    let mut src = String::new();
    src.push_str("#include <stdio.h>\n#include <stdlib.h>\n\n");
    for t in tests {
        src.push_str(&format!("extern int {}(void);\n", t.name));
    }
    src.push_str("\nint main(void) {\n");
    src.push_str("    int passed = 0, failed = 0;\n\n");
    for t in tests {
        src.push_str(&format!(
            "    if ({name}()) {{ printf(\"  ok  {name}\\n\"); passed++; }}\n",
            name = t.name
        ));
        src.push_str(&format!(
            "    else           {{ printf(\"  FAIL {name}\\n\"); failed++; }}\n",
            name = t.name
        ));
    }
    src.push_str("\n    printf(\"\\n%d passed, %d failed\\n\", passed, failed);\n");
    src.push_str("    return failed > 0 ? 1 : 0;\n");
    src.push_str("}\n");

    let harness = tmp.join("test_main.c");
    let _ = fs::write(&harness, &src);
    println!("test harness written to {}", harness.display());
    println!("compile and run manually:");
    println!("  cc -o test_runner {} <your_objects> && ./test_runner", harness.display());
}

// ── C++ test runner ───────────────────────────────────────────────────────────

fn run_cpp_tests(tests: &[TestFn], tmp: &Path) {
    let mut src = String::new();
    src.push_str("#include <iostream>\n\n");
    for t in tests {
        src.push_str(&format!("extern bool {}();\n", t.name));
    }
    src.push_str("\nint main() {\n");
    src.push_str("    int passed = 0, failed = 0;\n\n");
    for t in tests {
        src.push_str(&format!(
            "    if ({name}()) {{ std::cout << \"  ok  {name}\" << std::endl; passed++; }}\n",
            name = t.name
        ));
        src.push_str(&format!(
            "    else           {{ std::cout << \"  FAIL {name}\" << std::endl; failed++; }}\n",
            name = t.name
        ));
    }
    src.push_str("\n    std::cout << passed << \" passed, \" << failed << \" failed\" << std::endl;\n");
    src.push_str("    return failed > 0 ? 1 : 0;\n}\n");

    let harness = tmp.join("test_main.cpp");
    let _ = fs::write(&harness, &src);
    println!("test harness written to {}", harness.display());
    println!("compile and run manually:");
    println!("  c++ -std=c++17 -o test_runner {} <your_objects> && ./test_runner", harness.display());
}

// ── Python test runner ────────────────────────────────────────────────────────

fn run_python_tests(tests: &[TestFn], tmp: &Path) {
    let mut src = String::new();
    src.push_str("import unittest\n\n");
    // Each test module will need to be importable
    let modules: std::collections::HashSet<&str> = tests.iter().map(|t| t.module.as_str()).collect();
    for m in &modules {
        src.push_str(&format!("import {}\n", m));
    }
    src.push_str("\nclass BullangTests(unittest.TestCase):\n");
    for t in tests {
        src.push_str(&format!(
            "    def test_{name}(self):\n        self.assertTrue({module}.{name}())\n\n",
            name = t.name, module = t.module
        ));
    }
    src.push_str("if __name__ == '__main__':\n    unittest.main()\n");

    let harness = tmp.join("test_bullang.py");
    let _ = fs::write(&harness, &src);

    println!("running python tests...");
    let status = std::process::Command::new("python3")
        .arg(&harness)
        .status();

    if let Ok(s) = status {
        if !s.success() { std::process::exit(1); }
    }
}

// ── Go test runner ────────────────────────────────────────────────────────────

fn run_go_tests(tests: &[TestFn], tmp: &Path) {
    // Emit a *_test.go file using the testing package
    let mut src = String::new();
    src.push_str("package main\n\nimport \"testing\"\n\n");
    for t in tests {
        let go_name: String = {
            let mut s = t.name.clone();
            if let Some(r) = s.get_mut(0..1) { r.make_ascii_uppercase(); }
            s
        };
        src.push_str(&format!(
            "func Test{go_name}(t *testing.T) {{\n    if !{fn_name}() {{ t.Fail() }}\n}}\n\n",
            go_name = go_name, fn_name = t.name
        ));
    }

    let harness = tmp.join("bullang_test.go");
    let _ = fs::write(&harness, &src);
    println!("test harness written to {}", harness.display());
    println!("place alongside your generated Go files and run:");
    println!("  go test .");
}
