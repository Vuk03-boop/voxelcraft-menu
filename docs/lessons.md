# Lessons and fumbles log

Living log of pitfalls and agent mistakes. Rule: when the agent (or a human)
wastes a loop on a wrong assumption, append the entry that would have
prevented it. Keep entries to one line plus a pointer.

## Format

`- YYYY-MM-DD — what happened -> what to do instead (pointer)`

## Entries

- 2026-09-18 — Bulk edit rewrote every file's line endings -> the tree is
  deliberately mixed-EOL and `.gitattributes` pins `* -text`; detect per-file
  EOL and preserve it before rewriting a file. (`.gitattributes`)
- 2026-09-18 — Assumed `.gitattributes`' prose about which files are CRLF was
  current -> that comment is stale prose describing an older tree (it names
  long-gone docs); measure the bytes, never trust the comment.
- 2026-09-18 — Comment-stripping naively with regex would have eaten `//`
  inside string literals (several tests do `l.split("//")` on shader source)
  -> tokenize; strings/raw strings/char literals first. (`tests/support/mod.rs`)
- 2026-09-18 — `tests/bins.rs` looks like it pins `src/bin/*` tools, but no
  `src/bin/` exists and the suite early-returns on missing files -> it is
  vacuous in this tree; it is not evidence those binaries exist.
- 2026-09-18 — `--water-mottle` accepts a value and silently discards it
  (stderr notice, fixed 0.0) -> do not "fix" the plumbing; the pin is
  deliberate (ADR 0002).
- Standing — citing line numbers in docs/comments goes stale silently -> cite
  paths and symbols only. (`docs/README.md` rule 3)
