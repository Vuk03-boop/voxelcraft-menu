/// Extract a WGSL function after stripping line comments; braces are then code only.
pub fn wgsl_function(source: &str, name: &str) -> String {
    let code = source.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
    let pattern = format!("fn {name}(");
    let start = code.find(&pattern).expect("WGSL function exists");
    let open = start + code[start..].find('{').expect("function body");
    let mut depth = 0;
    for (offset, ch) in code[open..].char_indices() {
        if ch == '{' { depth += 1; }
        if ch == '}' { depth -= 1; if depth == 0 { return code[start..open+offset+1].into(); } }
    }
    panic!("unterminated WGSL function");
}
