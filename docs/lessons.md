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

## Source-text pins can pass on narration

`water_seen_through_a_pane_is_shaded_as_water` matched `WATER_BODY` and
`schlick(` inside the search window that spanned a *comment* above the call
site. The comment strip of 2026-09 flipped the test red while the code was
byte-identical: for years the pin had been reading the comment, not the
code. Source-text pins must anchor on the mechanism (route into the shared
entry, then assert each load-bearing word where it actually lives), and the
failure message should name *what* the invariant protects so the next
refactor sees the intent.

## A zero-pixel arm is a question, not an answer

The first full run of `validation/upload_report.py` produced sixteen
"zero-pixel" arms; every single one was the *scene* failing to reach the
feature, not the feature being dead: a value clamped on the CPU (`tint`), a
diagnostic that only applies to another diagnostic (`probe-noise` needs
`--probe-fill`), an underwater-only path sampled from the air (`snell`), a
window size that needs its switch on (`sun-softness` under `--soft-shadows`),
and demo structures buried inside the hill 14 blocks in front of the default
camera. Benchmark truth: pairing beats screenshots. Each arm now ships with
the vantage that exercises it (the same catalogue `src/harness/vantage.rs`
curates), or with an explicit "expected inert in this scene, because ..."
note. Never green-light a feature from one camera.
