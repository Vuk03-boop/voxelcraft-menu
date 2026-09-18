//! The guard batch 22c left behind.
//!
//! Splitting `CLAUDE.md` into a briefing plus `docs/roadmap.md`, `docs/ledger.md` and
//! `docs/errors.md` moved a few dozen relative links one directory deeper, and every
//! one of them broke silently -- a markdown link that points nowhere renders exactly like
//! one that does not. So the restructure ships with the check it needed: every relative
//! `](path)` in every `.md` in the tree must resolve. It found two pre-existing breaks the
//! first time it ran, which is the point of doing it mechanically rather than carefully.

use std::fs;
use std::path::{Path, PathBuf};

/// Every `.md` in the tree, `target/` excluded (3 GB of build output, none of it ours).
fn markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if path.is_dir() {
                if name != "target" && name != ".git" {
                    stack.push(path);
                }
            } else if name.ends_with(".md") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Blank out fenced blocks and inline code spans before looking for links.
///
/// Both matter here and for opposite reasons. A fence can hold a shell command with
/// parentheses in it; an inline span can hold the literal text `](path)`, which
/// `docs/ledger.md` and the roadmap both use to *describe* this check. Replacing
/// with spaces rather than deleting keeps byte offsets, so a reported line number is real.
fn strip_code(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = vec![b' '; bytes.len()];
    let mut i = 0;
    let mut at_line_start = true;
    while i < bytes.len() {
        if at_line_start && bytes[i..].starts_with(b"```") {
            // to the end of the closing fence's line, or the end of the file
            let mut j = i + 3;
            while j < bytes.len() {
                if bytes[j] == b'\n' && bytes[j + 1..].starts_with(b"```") {
                    j += 4;
                    while j < bytes.len() && bytes[j] != b'\n' {
                        j += 1;
                    }
                    break;
                }
                j += 1;
            }
            let j = j.min(bytes.len());
            for k in i..j {
                if bytes[k] == b'\n' {
                    out[k] = b'\n';
                }
            }
            i = j;
            at_line_start = true;
            continue;
        }
        if bytes[i] == b'`' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'`' && bytes[j] != b'\n' {
                j += 1;
            }
            // an unterminated span is not a span; fall through and keep the character
            if j < bytes.len() && bytes[j] == b'`' {
                i = j + 1;
                continue;
            }
        }
        out[i] = bytes[i];
        at_line_start = bytes[i] == b'\n';
        i += 1;
    }
    String::from_utf8(out).expect("stripping preserves byte boundaries")
}

/// `](...)` targets, with the line number each was found on.
fn link_targets(text: &str) -> Vec<(usize, String)> {
    let stripped = strip_code(text);
    let mut out = Vec::new();
    let bytes = stripped.as_bytes();
    let mut line = 1;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
        } else if bytes[i..].starts_with(b"](") {
            if let Some(end) = stripped[i + 2..].find(')') {
                out.push((line, stripped[i + 2..i + 2 + end].to_string()));
                i += 2 + end;
                continue;
            }
        }
        i += 1;
    }
    out
}

#[test]
fn every_relative_markdown_link_resolves() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = markdown_files(root);
    assert!(
        // A sanity bound on the walk, not a claim about the tree. It was `> 20`
        // against 21 files until the user's own `DO-NOT-READ-CLAUDE.md` was
        // deleted, which left the tree at exactly 20 and the guard red for
        // something it was never meant to have an opinion about.
        files.len() > 15,
        "found only {} markdown files -- the walk is broken, not the tree",
        files.len()
    );

    let mut broken = Vec::new();
    let mut checked = 0usize;
    for file in &files {
        let text = fs::read_to_string(file).expect("markdown is utf-8");
        let dir = file.parent().expect("a file has a parent");
        for (line, target) in link_targets(&text) {
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
                || target.starts_with('#')
            {
                continue;
            }
            // strip an anchor: the path half is what has to exist
            let path = target.split('#').next().unwrap_or("");
            if path.is_empty()
                || path.ends_with(".png")
                || path.ends_with(".jpg")
                || path.contains("research")
                || path.contains("src/bin/")
            {
                continue;
            }
            checked += 1;
            if !dir.join(path).exists() {
                let rel = file.strip_prefix(root).unwrap_or(file);
                broken.push(format!("{}:{} -> {}", rel.display(), line, target));
            }
        }
    }

    assert!(
        checked > 50,
        "only {checked} relative links found across {} files -- the parser is broken",
        files.len()
    );
    assert!(
        broken.is_empty(),
        "{} broken relative link(s):\n{}",
        broken.len(),
        broken.join("\n")
    );
}

/// The split only works if the briefing says where the rest went. Batch 22c's own risk is
/// that a later session trims these pointers and leaves three orphan files nothing names.
#[test]
fn the_briefing_points_at_the_five_files_split_out_of_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let briefing = fs::read_to_string(root.join("CLAUDE.md")).expect("CLAUDE.md is readable");
    for f in [
        "docs/roadmap.md",
        "docs/ledger.md",
        "docs/errors.md",
        // Batch 25. The two with a *timing* rather than a topic: pitfalls before you type a
        // command, lessons before you plan a batch. A briefing that stopped naming them
        // would leave 37% of what it used to say unreachable.
        "docs/pitfalls.md",
        "docs/lessons.md",
    ] {
        assert!(
            briefing.contains(&format!("]({f})")),
            "CLAUDE.md no longer links to {f}"
        );
        assert!(root.join(f).exists(), "{f} is missing");
    }
}

// ---------------------------------------------------------------------------
// Batch 28. The link guard above was right about the mechanism and too narrow
// about the scope: a documentation fact drifts exactly the way a link breaks,
// silently and without rendering differently. Four had already drifted when this
// was written -- README claimed 48 tests against CLAUDE.md's 82 (the tree had
// 82), 0.132 bytes per solid voxel against PERF.md's 0.130, and "the sea has no
// waves" in the section a reader trusts most, eleven batches after the waves
// shipped. Both files also claimed `--help` lists every flag while six were
// missing from it. None of those is catchable by reading carefully; all four are
// catchable by the checks below.
// ---------------------------------------------------------------------------

/// The help text is the home for "what flags exist". It has to actually be one.
///
/// This is the check that found the defect: `--bench-frames`, `--cam-height`,
/// `--cam-yaw`, `--cam-pitch`, `--demo-edits` and `--hud` were all parsed and
/// none were listed, while two documents asserted the list was complete. Three
/// of the six were documented *only* in the README paragraph batch 28 deleted,
/// so the cull would have taken their last mention with it.
#[test]
fn every_flag_the_parser_accepts_appears_in_the_help_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join("src/config.rs")).expect("config.rs is readable");

    let help_start = src
        .find(r#""--help" | "-h" =>"#)
        .expect("the help arm is still spelled this way");
    let help_end = src[help_start..]
        .find("std::process::exit(0);")
        .expect("the help arm still exits")
        + help_start;
    let help = &src[help_start..help_end];

    // Flag literals in the match arms -- everything before the help arm itself.
    let arms = &src[..help_start];
    let mut missing = Vec::new();
    let mut checked = 0usize;
    for (i, _) in arms.match_indices("\"--") {
        let rest = &arms[i + 1..];
        let end = match rest.find('"') {
            Some(e) => e,
            None => continue,
        };
        let flag = &rest[..end];
        // a flag is lowercase, digits and dashes; anything else is prose in a string
        if flag.len() < 3
            || !flag[2..]
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            continue;
        }
        checked += 1;
        if !help.contains(flag) {
            missing.push(flag.to_string());
        }
    }

    assert!(
        checked > 50,
        "only {checked} flag literals found in config.rs -- the scan is broken, not the help"
    );
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "{} flag(s) parsed but absent from --help:\n  {}\n\
         Both CLAUDE.md and README.md tell the reader --help is the complete list.",
        missing.len(),
        missing.join("\n  ")
    );
}

/// A count only `cargo test` knows must not be transcribed into prose.
///
/// README said 48 and CLAUDE.md said 82 for the same command. Neither number is
/// worth maintaining by hand, so neither is allowed to exist.
#[test]
fn no_document_transcribes_the_test_count() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for file in markdown_files(root) {
        let text = fs::read_to_string(&file).expect("markdown is utf-8");
        for (n, line) in text.lines().enumerate() {
            // "<digits> tests" anywhere in the line
            let Some(at) = line.find(" tests") else {
                continue;
            };
            let head = line[..at].trim_end();
            if head.chars().next_back().is_some_and(|c| c.is_ascii_digit()) {
                let rel = file.strip_prefix(root).unwrap_or(&file);
                found.push(format!("{}:{} -> {}", rel.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "a hand-maintained test count is in {} place(s):\n{}\n\
         Say what the tests cover, not how many there are.",
        found.len(),
        found.join("\n")
    );
}

/// `PERF.md` is the only home for a measurement, because it is the only file
/// that re-measures. README quoted 0.132 bytes per solid voxel against PERF.md's
/// 0.130 -- one drifted transcription out of six, and no way to tell which
/// without opening both.
#[test]
fn the_readme_quotes_no_measurement_from_perf_md() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("README.md is readable");
    let perf = fs::read_to_string(root.join("PERF.md")).expect("PERF.md is readable");

    let mut bad = Vec::new();

    // Every frame rate PERF.md reports, which is every frame rate that can drift.
    let mut rates: Vec<&str> = Vec::new();
    for (i, _) in perf.match_indices(" fps") {
        let head = perf[..i].trim_end();
        let start = head
            .rfind(|c: char| !c.is_ascii_digit())
            .map_or(0, |p| p + 1);
        if start < head.len() {
            rates.push(&head[start..]);
        }
    }
    rates.sort_unstable();
    rates.dedup();
    for rate in &rates {
        if readme.contains(&format!("{rate} fps")) {
            bad.push(format!("frame rate `{rate} fps` is PERF.md's to quote"));
        }
    }
    assert!(
        !rates.is_empty(),
        "no frame rates found in PERF.md -- the scan is broken, not the README"
    );

    // A per-pass or per-feature cost, in the shape those are always written.
    for (i, _) in readme.match_indices(" ms") {
        let head = readme[..i].trim_end();
        if head.chars().next_back().is_some_and(|c| c.is_ascii_digit()) {
            let start = head
                .rfind(|c: char| !c.is_ascii_digit() && c != '.')
                .map_or(0, |p| p + 1);
            bad.push(format!(
                "timing `{} ms` belongs in PERF.md",
                &head[start..]
            ));
        }
    }

    // The storage figure, which is the one that actually drifted.
    if readme.contains("bytes per solid voxel") || readme.contains("bytes/solid") {
        bad.push("the bytes-per-solid-voxel figure belongs in PERF.md".to_string());
    }

    bad.sort();
    bad.dedup();
    assert!(
        bad.is_empty(),
        "README.md transcribes {} measurement(s):\n  {}",
        bad.len(),
        bad.join("\n  ")
    );
}

/// Blank fenced code blocks and HTML comments, keeping everything else.
///
/// Deliberately *not* `strip_code`: inline spans are exactly where a flag
/// reference would be written, so blanking those would blind the budget below to
/// the thing it exists to catch.
fn strip_fences_and_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let fence = rest.find("```");
        let comment = rest.find("<!--");
        let (at, close, skip) = match (fence, comment) {
            (None, None) => break,
            (Some(f), Some(c)) if f < c => (f, "```", 3),
            (Some(_), Some(c)) => (c, "-->", 3),
            (Some(f), None) => (f, "```", 3),
            (None, Some(c)) => (c, "-->", 3),
        };
        out.push_str(&rest[..at]);
        let body = &rest[at + skip..];
        rest = match body.find(close) {
            Some(e) => &body[e + close.len()..],
            // unterminated: the rest of the file is inside it
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// The README must not grow a flag reference back.
///
/// It carried one for twenty-odd batches: thirty-two lines of prose naming 48
/// flags, against CLAUDE.md's table naming 43 -- two overlapping sets, neither a
/// superset, and the prose carried neither the cost nor the clears that make the
/// table worth reading. This is a budget rather than a semantic check, and it is
/// deliberately loose: the README may reach for a handful of flags by name while
/// describing something else. It may not become the list again.
#[test]
fn the_readme_does_not_become_a_flag_reference() {
    const BUDGET: usize = 20;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("README.md is readable");

    // Fenced blocks are commands to run and the HTML comment is the screenshot
    // recipe; both *use* flags rather than documenting them, and counting them
    // would put the budget permanently over on content nobody reads as a list.
    let prose = strip_fences_and_comments(&readme);

    let mut flags: Vec<&str> = Vec::new();
    for (i, _) in prose.match_indices("--") {
        let rest = &prose[i..];
        let end = rest
            .find(|c: char| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-')
            .unwrap_or(rest.len());
        let flag = &rest[..end];
        if flag.len() > 3 && flag[2..].starts_with(|c: char| c.is_ascii_lowercase()) {
            flags.push(flag);
        }
    }
    flags.sort_unstable();
    flags.dedup();

    assert!(
        flags.len() <= BUDGET,
        "README.md names {} distinct flags, over the budget of {BUDGET}:\n  {}\n\
         The flag reference is `--help` and the cost table is CLAUDE.md's.",
        flags.len(),
        flags.join(" ")
    );
}



