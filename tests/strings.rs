//! Guard against string-literal corruption that stops the crate from parsing.
//!
//! History: the dump pipeline that produced this repository's initial commit
//! split `\n` escapes into physical newlines and dropped line-continuation
//! backslashes, leaving 30 sites across 8 files where a normal string literal
//! spanned lines. rustc rejects every one of them, so the tree could not
//! compile at all. The sites were repaired by re-escaping; this test exists so
//! that class of damage cannot land again unnoticed.
//!
//! What counts as a violation:
//!   * a physical line ending while a normal (non-raw) string literal is open,
//!     without a trailing `\` continuation;
//!   * a `\` escape inside a string naming a character Rust does not accept.
//!
//! Raw strings (`r#"..."#`) may span lines by design and are not flagged.

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn normal_string_literals_stay_on_one_line() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rs(&root.join("src"), &mut files);
    collect_rs(&root.join("tests"), &mut files);
    collect_rs(&root.join("benches"), &mut files);
    assert!(!files.is_empty(), "found no .rs files to guard");

    let mut bad: Vec<String> = Vec::new();
    for file in &files {
        let src = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => panic!("cannot read {}: {e}", file.display()),
        };
        scan(&src, file, &mut bad);
    }
    assert!(
        bad.is_empty(),
        "string literals that cannot parse ({} found):\n  {}",
        bad.len(),
        bad.join("\n  ")
    );
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// A single-pass lexer for the one question that matters here: does any normal
/// string literal stay open across a line break, or use an escape Rust
/// rejects? It is deliberately tolerant of everything else -- comments,
/// lifetimes, char literals and raw strings are all handled so that valid
/// code is never flagged.
fn scan(src: &str, file: &Path, bad: &mut Vec<String>) {
    const CODE: u8 = 0;
    const LINE_COMMENT: u8 = 1;
    const BLOCK_COMMENT: u8 = 2;
    const STRING: u8 = 3;
    const CHAR: u8 = 4;
    const RAW: u8 = 5;

    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let at = |k: usize| -> char { if k < n { chars[k] } else { '\0' } };

    let mut i = 0usize;
    let mut line = 1usize;
    let mut state = CODE;
    let mut raw_hashes = 0usize;
    let mut block_depth = 0usize;

    while i < n {
        let c = chars[i];
        match state {
            CODE => {
                if c == '/' && at(i + 1) == '/' {
                    state = LINE_COMMENT;
                    i += 2;
                } else if c == '/' && at(i + 1) == '*' {
                    state = BLOCK_COMMENT;
                    block_depth = 1;
                    i += 2;
                } else if c == 'r' {
                    // Raw string: `r"..."` or `r#"..."#` with any number of hashes.
                    let mut j = i + 1;
                    let mut hashes = 0;
                    while j < n && chars[j] == '#' {
                        hashes += 1;
                        j += 1;
                    }
                    if j < n && chars[j] == '"' {
                        state = RAW;
                        raw_hashes = hashes;
                        i = j + 1;
                    } else {
                        i += 1;
                    }
                } else if c == '"' {
                    state = STRING;
                    i += 1;
                } else if c == '\'' {
                    // Char literal (`'x'`, `'\n'`) or lifetime (`'a`) -- a
                    // lifetime is skipped, a char literal enters CHAR state.
                    if at(i + 1) == '\\' || at(i + 2) == '\'' {
                        state = CHAR;
                    }
                    i += 1;
                } else if c == '\n' {
                    line += 1;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            LINE_COMMENT => {
                if c == '\n' {
                    state = CODE;
                    line += 1;
                }
                i += 1;
            }
            BLOCK_COMMENT => {
                if c == '/' && at(i + 1) == '*' {
                    block_depth += 1;
                    i += 2;
                } else if c == '*' && at(i + 1) == '/' {
                    block_depth -= 1;
                    if block_depth == 0 {
                        state = CODE;
                    }
                    i += 2;
                } else {
                    if c == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
            RAW => {
                if c == '"' {
                    let mut k = 0;
                    while k < raw_hashes && at(i + 1 + k) == '#' {
                        k += 1;
                    }
                    if k == raw_hashes {
                        i += 1 + raw_hashes;
                        state = CODE;
                        continue;
                    }
                }
                if c == '\n' {
                    line += 1;
                }
                i += 1;
            }
            STRING => {
                if c == '\n' {
                    // A string may only span lines via a `\` continuation
                    // immediately before the line break.
                    let mut j = i as isize - 1;
                    if j >= 0 && chars[j as usize] == '\r' {
                        j -= 1;
                    }
                    if j >= 0 && chars[j as usize] == '\\' {
                        line += 1;
                        i += 1;
                        while i < n && matches!(chars[i], ' ' | '\t' | '\r' | '\n') {
                            if chars[i] == '\n' {
                                line += 1;
                            }
                            i += 1;
                        }
                    } else {
                        bad.push(format!(
                            "{}:{}: raw newline inside a normal string literal",
                            file.display(),
                            line
                        ));
                        line += 1;
                        i += 1;
                        // Stay in STRING so the closing quote still balances
                        // and later breaks in the same literal are also found.
                    }
                } else if c == '\\' {
                    match at(i + 1) {
                        'n' | 'r' | 't' | '0' | '\\' | '\'' | '"' | 'x' | 'u' => i += 2,
                        '\n' | '\r' => {
                            // Line-continuation escape: skip the newline and
                            // all following whitespace, as rustc does.
                            i += 1;
                            while i < n && matches!(chars[i], ' ' | '\t' | '\r' | '\n') {
                                if chars[i] == '\n' {
                                    line += 1;
                                }
                                i += 1;
                            }
                        }
                        e => {
                            bad.push(format!(
                                "{}:{}: invalid escape \\{} in string literal",
                                file.display(),
                                line,
                                e
                            ));
                            i += 2;
                        }
                    }
                } else if c == '"' {
                    state = CODE;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            CHAR => {
                if c == '\\' {
                    i += 2;
                } else if c == '\'' {
                    state = CODE;
                    i += 1;
                } else if c == '\n' {
                    // A char literal cannot span lines; recover at the break.
                    state = CODE;
                    line += 1;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            _ => unreachable!(),
        }
    }
}
