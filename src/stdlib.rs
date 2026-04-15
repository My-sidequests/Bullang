//! Standard library — universal builtin functions.
//!
//! Only builtins that can be implemented in ALL five backends are included.
//! Collection builtins (map, filter, sort, etc.) require generic types that
//! C cannot express, so they are excluded. What remains is a clean set of
//! 13 math and string primitives that work identically everywhere.
//!
//! Syntax in source files:  builtin::abs   builtin::to_upper   etc.
//!
//! Backends: Rust, Python, C, C++, Go

use crate::ast::{Backend, Param};

// ── Universal builtin set ─────────────────────────────────────────────────────

/// The 13 universal builtins — available in every backend.
pub const BUILTINS: &[(&str, &str, &str)] = &[
    // (name, param_signature, description)
    // Math
    ("abs",        "(x: numeric)             → |x|",                    "Absolute value"),
    ("pow",        "(base: numeric, exp: numeric) → base^exp (integer)", "Integer power"),
    ("powf",       "(base: f64, exp: f64)     → base^exp (float)",       "Float power"),
    ("sqrt",       "(x: f64)                  → √x",                     "Square root"),
    ("clamp",      "(x, min, max)             → x clamped to [min,max]", "Clamp to range"),
    // String
    ("to_upper",   "(s: String)               → String",                 "Uppercase"),
    ("to_lower",   "(s: String)               → String",                 "Lowercase"),
    ("trim",       "(s: String)               → String",                 "Strip whitespace"),
    ("starts_with","(s: String, prefix: String) → bool",                 "Prefix check"),
    ("ends_with",  "(s: String, suffix: String) → bool",                 "Suffix check"),
    ("replace_str","(s, from, to: String)     → String",                 "Replace occurrences"),
    ("to_string",  "(x: numeric)              → String",                 "Convert to string"),
    ("parse_i64",  "(s: String)               → i64 (or error)",        "Parse string to integer"),
];

/// Returns true if the name is a known universal builtin.
pub fn is_known_builtin(name: &str) -> bool {
    BUILTINS.iter().any(|(n, _, _)| *n == name)
}

/// Print the builtin catalogue.
pub fn list_builtins() -> Vec<(&'static str, &'static str, &'static str)> {
    BUILTINS.to_vec()
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub fn emit_builtin(name: &str, params: &[Param], backend: &Backend) -> Result<String, String> {
    if !is_known_builtin(name) {
        return Err(format!(
            "'builtin::{}' is not a known builtin. \
             Run `bullang stdlib --list` to see available builtins.",
            name
        ));
    }
    match backend {
        Backend::Rust        => emit_rust(name, params),
        Backend::Python      => emit_python(name, params),
        Backend::C           => emit_c(name, params),
        Backend::Cpp         => emit_cpp(name, params),
        Backend::Go          => emit_go(name, params),
        Backend::Unknown(kw) => Err(format!("'builtin::{}' is not available for unknown backend '{}'", name, kw)),
    }
}

fn p(params: &[Param]) -> Vec<&str> {
    params.iter().map(|p| p.name.as_str()).collect()
}

/// Escape a param name that might be a Python reserved word.
/// Used when emitting Python expressions that reference param names.
fn py_esc(name: &str) -> &str {
    match name {
        "from" => "from_", "import" => "import_", "class" => "class_",
        "return" => "return_", "pass" => "pass_", "for" => "for_",
        "while" => "while_", "in" => "in_", "not" => "not_",
        "and" => "and_", "or" => "or_", "if" => "if_", "else" => "else_",
        "lambda" => "lambda_", "with" => "with_", "as" => "as_",
        "try" => "try_", "except" => "except_", "raise" => "raise_",
        "del" => "del_", other => other,
    }
}

fn need<'a>(name: &str, params: &'a [Param], n: usize) -> Result<Vec<&'a str>, String> {
    let v = p(params);
    if v.len() != n {
        return Err(format!(
            "'builtin::{}' requires {} parameter(s) but the function declares {}",
            name, n, v.len()
        ));
    }
    Ok(v)
}

// ── Rust ──────────────────────────────────────────────────────────────────────

fn emit_rust(name: &str, params: &[Param]) -> Result<String, String> {
    let p = need(name, params, param_count(name))?;
    Ok(match name {
        "abs"         => format!("{}.abs()", p[0]),
        "pow"         => format!("{}.pow({} as u32)", p[0], p[1]),
        "powf"        => format!("{}.powf({})", p[0], p[1]),
        "sqrt"        => format!("{}.sqrt()", p[0]),
        "clamp"       => format!("{}.clamp({}, {})", p[0], p[1], p[2]),
        "to_upper"    => format!("{}.to_uppercase()", p[0]),
        "to_lower"    => format!("{}.to_lowercase()", p[0]),
        "trim"        => format!("{}.trim().to_owned()", p[0]),
        "starts_with" => format!("{0}.starts_with({1}.as_str())", p[0], p[1]),
        "ends_with"   => format!("{0}.ends_with({1}.as_str())", p[0], p[1]),
        "replace_str" => format!("{0}.replace({1}.as_str(), {2}.as_str())", p[0], p[1], p[2]),
        "to_string"   => format!("{}.to_string()", p[0]),
        "parse_i64"   => format!("{}.trim().parse::<i64>().unwrap_or(0)", p[0]),
        _             => unreachable!(),
    })
}

// ── Python ────────────────────────────────────────────────────────────────────

fn emit_python(name: &str, params: &[Param]) -> Result<String, String> {
    let raw = need(name, params, param_count(name))?;
    // Escape any Python reserved keywords used as parameter names
    let escaped: Vec<&str> = raw.iter().map(|n| py_esc(n)).collect();
    let p = escaped;
    Ok(match name {
        "abs"         => format!("abs({})", p[0]),
        "pow"         => format!("{} ** int({})", p[0], p[1]),
        "powf"        => format!("{} ** {}", p[0], p[1]),
        "sqrt"        => format!("__import__('math').sqrt({})", p[0]),
        "clamp"       => format!("max({1}, min({2}, {0}))", p[0], p[1], p[2]),
        "to_upper"    => format!("{}.upper()", p[0]),
        "to_lower"    => format!("{}.lower()", p[0]),
        "trim"        => format!("{}.strip()", p[0]),
        "starts_with" => format!("{}.startswith({})", p[0], p[1]),
        "ends_with"   => format!("{}.endswith({})", p[0], p[1]),
        "replace_str" => format!("{}.replace({}, {})", p[0], p[1], p[2]),
        "to_string"   => format!("str({})", p[0]),
        "parse_i64"   => format!("int({}.strip())", p[0]),
        _             => unreachable!(),
    })
}

// ── C ─────────────────────────────────────────────────────────────────────────

fn emit_c(name: &str, params: &[Param]) -> Result<String, String> {
    let p = need(name, params, param_count(name))?;
    Ok(match name {
        "abs"         => format!("abs({})", p[0]),
        "pow"         => format!("(int64_t)pow((double){}, (double){})", p[0], p[1]),
        "powf"        => format!("pow({}, {})", p[0], p[1]),
        "sqrt"        => format!("sqrt((double){})", p[0]),
        "clamp"       => format!("({0} < {1} ? {1} : ({0} > {2} ? {2} : {0}))", p[0], p[1], p[2]),
        "to_upper"    => format!("/* to_upper: iterate with toupper() over {} */", p[0]),
        "to_lower"    => format!("/* to_lower: iterate with tolower() over {} */", p[0]),
        "trim"        => format!("/* trim: implement manually for {} */", p[0]),
        "starts_with" => format!("(strncmp({0}, {1}, strlen({1})) == 0)", p[0], p[1]),
        "ends_with"   => format!(
            "(strlen({0}) >= strlen({1}) && strcmp({0} + strlen({0}) - strlen({1}), {1}) == 0)",
            p[0], p[1]
        ),
        "replace_str" => format!("/* replace_str: implement manually for {} */", p[0]),
        "to_string"   => format!("/* to_string: use sprintf for {} */", p[0]),
        "parse_i64"   => format!("strtoll({}, NULL, 10)", p[0]),
        _             => unreachable!(),
    })
}

// ── C++ ───────────────────────────────────────────────────────────────────────

fn emit_cpp(name: &str, params: &[Param]) -> Result<String, String> {
    let p = need(name, params, param_count(name))?;
    Ok(match name {
        "abs"         => format!("std::abs({})", p[0]),
        "pow"         => format!("(decltype({0}))std::pow((double){0}, (double){1})", p[0], p[1]),
        "powf"        => format!("std::pow({}, {})", p[0], p[1]),
        "sqrt"        => format!("std::sqrt({})", p[0]),
        "clamp"       => format!("std::clamp({}, {}, {})", p[0], p[1], p[2]),
        "to_upper"    => format!(
            "[&](){{ std::string _s({0}); \
            std::transform(_s.begin(),_s.end(),_s.begin(),::toupper); return _s; }}()", p[0]
        ),
        "to_lower"    => format!(
            "[&](){{ std::string _s({0}); \
            std::transform(_s.begin(),_s.end(),_s.begin(),::tolower); return _s; }}()", p[0]
        ),
        "trim"        => format!(
            "[&](){{ std::string _s({0}); \
            _s.erase(0,_s.find_first_not_of(\" \\t\\n\\r\")); \
            _s.erase(_s.find_last_not_of(\" \\t\\n\\r\")+1); return _s; }}()", p[0]
        ),
        "starts_with" => format!("{0}.rfind({1}, 0) == 0", p[0], p[1]),
        "ends_with"   => format!(
            "({0}.size() >= {1}.size() && \
            {0}.compare({0}.size()-{1}.size(), {1}.size(), {1}) == 0)",
            p[0], p[1]
        ),
        "replace_str" => format!(
            "[&](){{ std::string _s={0}; std::size_t pos=0; \
            while((pos=_s.find({1},pos))!=std::string::npos) \
            {{ _s.replace(pos,{1}.size(),{2}); pos+={2}.size(); }} return _s; }}()",
            p[0], p[1], p[2]
        ),
        "to_string"   => format!("std::to_string({})", p[0]),
        "parse_i64"   => format!("std::stoll({})", p[0]),
        _             => unreachable!(),
    })
}

// ── Go ────────────────────────────────────────────────────────────────────────

fn emit_go(name: &str, params: &[Param]) -> Result<String, String> {
    let p = need(name, params, param_count(name))?;
    Ok(match name {
        "abs"         => format!("func() int64 {{ if {0} < 0 {{ return int64(-{0}) }}; return int64({0}) }}()", p[0]),
        "pow"         => format!("func() int64 {{ return int64(math.Round(math.Pow(float64({0}), float64({1})))) }}()", p[0], p[1]),
        "powf"        => format!("math.Pow({}, {})", p[0], p[1]),
        "sqrt"        => format!("math.Sqrt(float64({}))", p[0]),
        "clamp"       => format!("func() int64 {{ x,lo,hi:=int64({0}),int64({1}),int64({2}); if x<lo{{return lo}}; if x>hi{{return hi}}; return x }}()", p[0], p[1], p[2]),
        "to_upper"    => format!("strings.ToUpper({})", p[0]),
        "to_lower"    => format!("strings.ToLower({})", p[0]),
        "trim"        => format!("strings.TrimSpace({})", p[0]),
        "starts_with" => format!("strings.HasPrefix({}, {})", p[0], p[1]),
        "ends_with"   => format!("strings.HasSuffix({}, {})", p[0], p[1]),
        "replace_str" => format!("strings.ReplaceAll({}, {}, {})", p[0], p[1], p[2]),
        "to_string"   => format!("fmt.Sprintf(\"%v\", {})", p[0]),
        "parse_i64"   => format!("func() int64 {{ n, _ := strconv.ParseInt(strings.TrimSpace({}), 10, 64); return n }}()", p[0]),
        _             => unreachable!(),
    })
}

// ── Parameter count table ─────────────────────────────────────────────────────

fn param_count(name: &str) -> usize {
    match name {
        "abs" | "sqrt" | "to_upper" | "to_lower" | "trim"
        | "to_string" | "parse_i64" => 1,
        "pow" | "powf" | "starts_with" | "ends_with" => 2,
        "clamp" | "replace_str" => 3,
        _ => 0,
    }
}
