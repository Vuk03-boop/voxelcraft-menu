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

## A diff against a different camera measures the camera, not the flag

The first repaired run of `validation/upload_report.py` paired scene-context
arms with their curated vantages but still diffed them against the *default*
baseline — so every paired arm "passed" at 100% of frame by construction and
said nothing about the feature it was meant to watch. Any diff-based guard
needs an explicit statement of what baseline removes what variable; here the
answer is a cached context baseline per distinct context (same pins, flag
left off), and an import-time assert that every CTX arm's args end with its
declared context. SUSPECT lost teeth for exactly one run; caught by reading
the diffs instead of celebrating the green. Also closed by the same run:
the water-sec perf anomalies were thermal ordering all along (drift ±5.4%
with corrected re-run; scale-1.25's real number is +31%, expected).

## "Zero pixels" twice: read the plumbing before renaming the test

Two `SUSPECT` rows from the first paired-context run taught opposite lessons
in the same hour. `isolate-glass` looked like dead plumbing — flag bit set,
override declared, zero pixels — but the lane-split lives in `RESOLVE_LANE`
wiring in `render/mod.rs` (a sed window had cut off the pair list) and the
zero pixels *are* the contract: it is an output-preserving A/B pipeline, so
it belongs in EXACT with `compact-shade-hit`, with a perf arm to say why it
exists. `no-snell` was genuinely scene-gated, but by a constant, not a
camera: `SNELL_MIN_DEPTH = 3.0` blocks makes a one-block-deep periscope
identical on and off; the context needed submerge 5, not a new test. The
guard against the first failure mode now lives in tests/spec_hi.rs: every
`SPEC_` override with a `FLAG_HI_` sibling must be wired, and every wired
name must exist — dead by construction is the failure regexes find quietest.
