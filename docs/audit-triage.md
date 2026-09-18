# Audit triage: four external reports, adjudicated against this tree

Status: **verified findings only.** This file is the merged, deduplicated result of four
externally produced audits of this codebase, re-checked line by line against this checkout
rather than against the text dump they were written from. Anything I could not confirm in
this tree is either tagged `[unverified]` or parked in §5 with the reason.

## 0. Provenance and method

The four documents, and how they rate once checked:

| Report | What it was | Verdict |
|---|---|---|
| `codebase_error_report` | mechanical audit, whole tree | **most valuable.** Only report that found a live engine bug (§1.1). Nearly every claim verifies at the cited line. |
| `codebase-audit` (dropped twice, identical) | mechanical audit + confidence column | **best reasoning.** Its `EXPERIMENTS.md` recipe analysis (§2.2) is the highest-stakes finding in the pile. |
| `voxelcraft_audit` | mechanical audit, docs-focused | **best breadth, cheapest fixes.** Only report that found the table breakage (§4.1). Its hedge on `src/bin` was the correct call; the other two guessed wrong. |
| `Deconstructing the Engine` | narrative description, no findings | **not an audit.** Zero defects, one mis-attributed number (§5.3). |

Method used here: read the source at every cited location. Line numbers below are **this
tree's**, which run one line higher than the reports cite for `CLAUDE.md` and the docs
(their reconstructed tree had a one-line offset; their content citations were exact).

Not available in this environment, so not re-derived here: no `cargo`, no `rustc`, no GPU.
Claims that need a build or a device are marked and left as the original author's
measurement, not this file's.

## 1. Code defects

### 1.1 `march_chunk` is handed an unguarded direction by every secondary-tracer caller

`[verified]` `src/render/shaders/common.wgsl:2315` builds an epsilon-ized `dir` so the
chunk-selection DDA stays finite, uses it at `:2316` (`let inv = 1.0 / dir`), then at
**:2339** calls `march_chunk(ci, ro, rd, …)` with the raw `rd`. The same raw handoff is at
**:2135** (`trace_local`) and **:2238** (`shadow_ray_t`). Inside `march_chunk` the DDA does
`let inv = 1.0 / rd;` at **:1381**, so a zero component gives `±inf`, the comparison ladder
selects the degenerate axis, and `pos = ro + rd * (±inf)` produces `0 * inf` = `NaN`.

This is the mechanism behind the retained failing fixture
`validation/glass-transport-results.txt` (`AssertionError: (3.5, 0.0, 512.0, 0.0)`): the
primary hit survives because the entry cell is solid, and the first DDA step is where an
axis-aligned transmission leg dies.

The guard is not missing machinery. `ray_dir` (`common.wgsl:1006`) already states the house
rule -- "keep every component non-zero so reciprocals stay finite" -- so the primary path is
safe and only the secondary legs leak an unguarded `rd`. Two call sites remain for the same
shape: `shadow_ray` at `:2106` and `shadow_ray_t` at `:2222` each compute `1.0 / rd` for
their own selection DDA without the guard.

Fix: pass the epsilon-ized direction into the marcher at all three world-level sites, or
epsilon-ize on entry to `march_chunk` so its other direct callers
(`src/render/shaders/march.wgsl:30`, `:66`) are covered too.

### 1.2 `shadow_ray_t` lacks the water-surface skip `shadow_ray` has

`[verified]` `shadow_ray` (`common.wgsl:2091`) skips the sea-plane interface under
`SPEC_WATER_REPAIR` at **:2098** (`rd.y > 1e-4 && ro.y < frame.sea_level`). `shadow_ray_t`
(**:2221**), the tracer the dedicated shadow pass uses, has no equivalent branch. The split
is documented as a scheduling change, not a semantic one, so a submerged receiver can gain
an occluder in one arm of an A/B the docs call bit-exact.

### 1.3 Two settings are unreachable, and one cannot be switched off

`[verified]` `src/menu.rs:54-57` renders exactly two rows for `Page::Experimental`
(`CompactShade`, `SoftShadows`). `Setting::IsolateGlass` and `Setting::ShadowPass` still
exist in the enum (`:12`), in `experimental()` (`:15`), and in `adjust()` (`:110`, `:111`),
and are round-tripped by `encode`/`decode`, but no row ever emits an `Action::Adjust` for
them. They are dead plumbing.

`src/menu.rs:111` is the trap inside the dead code:

    Setting::ShadowPass=>p.shadow_pass=true,

while all its neighbours use `=!p.x`. If the row is ever wired up, the shadow pass turns on
and never turns off. One-character fix, do it before anything re-exposes the row.

### 1.4 Spec constants with no reader

`[verified]` `common.wgsl:2199` `override SOFT_TAPS: u32 = 3u` is supplied by the override
table (`src/render/mod.rs:4060`, 7.0/3.0 on `FLAG_HI_SOFT_HQ`) and read by no shader.
`common.wgsl:2210` `const SOFT_MISS_DIST: f32 = 64.0` is referenced nowhere.
`SOFT_PENUMBRA` (`:2188`) is live but for a different job -- a screen-space radius multiplier
at `src/render/shaders/shadow.wgsl:54`. `common.wgsl:2263` carries an orphan comment
describing "the centre bit plus `SOFT_TAPS` cone taps" over a body that no longer exists.
Net: one wasted specialization dimension and a comment that misdescribes shipped code.

## 2. Documentation that contradicts the code

### 2.1 `--no-shadow-pass` is sold as a revert and is not one

`[verified]` `src/config.rs:974-975`:

    "--shadow-pass"    => cfg.shadow_pass = true,
    "--no-shadow-pass" => { cfg.shadow_pass = true; eprintln!("--no-shadow-pass retired: ..."); },

`README.md:68` still tells the reader to "revert with their `--no-` counterparts", which is
false for this one member of the set, and contradicts the framing two paragraphs earlier at
`README.md:105` that each flag reproduces the pre-feature build bit for bit.
`EXPERIMENTS.md:17` lists `--no-shadow-pass` in the "CLI revert" column.

### 2.2 Every sample A/B recipe in `EXPERIMENTS.md` runs with the shadow pass on

`[verified]` `EXPERIMENTS.md:118-126` builds its control and split arms out of
`--no-isolate-glass --no-shadow-pass` and `--isolate-glass --shadow-pass`. Because
`--no-shadow-pass` sets `true` (§2.1), **both** arms have the dedicated pass enabled, so the
`control.png`/`split.png` pair presented as combined-vs-split is not that comparison, and
the architecture table's `Off`/`Off` row is unreachable. Highest-stakes finding in the pile:
it does not just misdescribe the build, it retroactively voids any measurement taken with
these commands. Audit each recorded result against the flags it actually ran with.

### 2.3 "Seven shader experiments off by default" is false

`[verified]` `README.md:64` ("The seven shader experiments are **off by default and
title-only**"), `EXPERIMENTS.md:7`, `MENU-NOTES.md:13`, `MENU-NOTES.md:60`. Reality:
`shadow_pass: true` at `src/config.rs:691`, `shadow_pass: true` hardcoded into the encode at
`src/settings.rs:58`, forced again at `src/settings.rs:90`, and re-forced unconditionally at
the end of `Preferences::decode` at `src/settings.rs:167` -- which silently overrides a file
that says `shadow_pass=false`. This is deliberate (`BATCH-LOG.md` DS01 makes the dedicated
path mandatory). The defect is the unedited docs.

### 2.4 The probe lattice's `SPACING 8 -> 4` never reached the prose

`[verified]` The authority: `pub const SPACING: i32 = 4` at `src/probe.rs:107`,
`const PROBE_SPACING: f32 = 4.0` at `resolve.wgsl:285`, and `PROBE_DIM_XZ = 512 / SPACING`
at `src/render/mod.rs:3568`. Statements still derived from 8:

| Site | Says | Should say |
|---|---|---|
| `src/probe.rs:1` | "a six-axis ambient cube every 8 blocks" | 4 |
| `src/probe.rs:437` | "20,480 height samples per chunk and eight times as many" | 16 probes, 16x |
| `src/probe.rs:693` | "the chunk's 20,480 height samples stay 20,480" | 81,920 |
| `src/stream.rs:22` | "a probe eight blocks inside a chunk" | four blocks |
| `src/render/mod.rs:1827` | "an eight-block band of wrong shading" | four-block |
| `src/render/mod.rs:3508` | "`WORLD_HEIGHT / SPACING` is 64 on the nose" | 128 |
| `src/render/mod.rs:3516` | "a shading factor, eight blocks wide" | four blocks |
| `common.wgsl:218` | "an eight-block band of wrong shading" | four-block |
| `docs/adr/0002-no-disc-sampled-sun.md:114` | "`TEXEL = 4` against `SPACING = 8`" | 4 against 4 -- the win is gone |
| `CLAUDE.md:147` | "8 blocks per probe" | 4 |
| `CLAUDE.md:152`, `:550` | "20,480 height samples stay 20,480" | 81,920 |
| `CLAUDE.md:155` | "is 64 x 384 x 64 since batch 57" | 128 x 768 x 128 |
| `CLAUDE.md:393` | "one probe per 8 blocks" | 4 |
| `CLAUDE.md:506` | "probe field 12.0 … 64 x 384 x 64 … cannot drift" | 96 MiB. It drifted, in the row that claims it cannot |
| `CLAUDE.md:625` | "384 in its largest axis … forty times the headroom" | 768, about 21x |

Historical rows are **not** errors and must not be "fixed": `PERF.md:338-339`, `:377`,
`docs/ledger.md` batch 57/59/93 rows are dated records under this repo's own vantage
convention. Fix the present-tense statements only.

### 2.5 The VRAM row is internally inconsistent

`[verified]` `CLAUDE.md:506` sums 66.4 + 8.4 + 3 + ~50 + 12.0 against a stated "~142 MB"
(~140 actual) and quotes "96.5% free" of 3.9 GB. With the shipped 96 MiB probe field the
total is ~231 MB, i.e. ~94% free. `docs/roadmap.md:1028-1029` has the right field size
(12.6 MB -> ~100.7 MB) while still carrying the inherited "96.5% free".

### 2.6 Flags documented as live that the parser rejects or discards

`[verified]` `--probe-sun` appears as a runnable control at `docs/lessons.md:74` and
`docs/shading.md:1061`, but `src/config.rs` knows only `--no-probe-sun` (`:1032`) and
`--probe-sun-high` (`:1033`); a reader following the doc hits an unknown-arg exit.
`--water-mottle` is parsed then force-zeroed with a warning at `src/config.rs:898` with the
default already `0.0` (`:666`), while `CLAUDE.md:118` and `docs/water.md:437` still list
`--water-mottle 0.2` as a free control -- and `docs/water.md:119` in the same file says the
layer ships flat. Note that a parser-to-`--help` sweep cannot catch either of these; only
docs-to-parser can.

### 2.7 Stale structural counts in code comments

`[verified]` `src/render/mod.rs:3673` says `common.wgsl` "and the four pass files" while
`:3683-3689` concatenates six (`tile_select`, `march`, `resolve`, `taa`, `shaft`, `shadow`).
`ENTRY_POINTS` is `[&str; 11]` (`:3696`) although `Renderer::new` also builds
`primary_shadow`, `shadow_horizontal`, `shadow_vertical` (`:4081-4082`), all three dispatched
-- so `shaderstats` reports no register count for exactly the passes the comment says the
list exists to prevent being missed. `src/render/mod.rs:1333-1334` says "these three pin the
two seams" above **seven** `offset_of!` asserts (`:1335-1341`).

### 2.8 Menu and experiment docs

`[verified]` `MENU-NOTES.md:26` lists four Render lab controls; the page has two (§1.3).
`EXPERIMENTS.md:55` still describes visibility as "full-resolution f32 values in a storage
buffer… not quantized to eight bits" and `:79` as "4 bytes per internal pixel: approximately
7.91 MiB at 1920x1080", while the shipped target is R8 quantized -- `validation/check_decoupled.py:88`
asserts mask values are exactly 0 or 255. The three shadow targets are R16F + RG16F + R8
(`src/render/shadow.rs:12-14`), i.e. 7 B/px all-in, 13.84 MiB at 1080p.

## 3. Test and tooling defects

### 3.1 The suite is machine-dependent

`[verified]` `settle()` is duplicated verbatim at `tests/lod_stream.rs:28-36` and
`tests/shadow_grid.rs:36-44`: a loop of `for _ in 0..6000` with a 200 us sleep, then
`panic!("world never settled")`. It is an **iteration** budget, not a time budget, so it
encodes a CPU-speed assumption. All three reports independently reproduced "world never
settled" in these two targets on 2-core boxes and watched the same world settle when given a
larger budget (~6.4 s, ~8 s and ~13.7 s on three different machines) -- the streaming logic
converges, the helper gives up. `docs/ledger.md` row 101-hw already closes it as a sandbox
artifact. Fix: one duration-capped helper in `tests/support/`, and a message reporting
"settle budget exhausted (N/M resident)" instead of asserting the engine is broken.

### 3.2 A guard that passes by returning early

`[verified]` `tests/bins.rs` reads each binary with
`let Some(h) = source("src/bin/harness.rs") else { return; };`, so all four tests report
green and assert nothing when the file is absent. A missing binary is currently
indistinguishable from a correct one. Same class of hole: `tests/settings_menu.rs`'s range
check iterates `m.rows()` only, so the two hidden settings of §1.3 are never validated.

### 3.3 A bound widened to fit a known defect

`[verified]` `tests/lod_stream.rs:336-338` asserts `ratio < 4.0` inside a test named
`transitions_are_symmetric_and_never_uncover`, with a comment recording that 4.0 was chosen
to accommodate a measured 3.23x-3.67x asymmetry. Either file the asymmetry in
`docs/errors.md` as open, or assert the real bound. Related: `tests/tree.rs:324` asserts
against `9 % 16`, which is identically 9 and constrains nothing.

### 3.4 Clippy's stated state is wrong

`[verified]` `CLAUDE.md:54` says clippy is "silent since batch 22; keep it that way". 13
instances of the `x=!y` shape that `suspicious_assignment_formatting` flags are present in
`src/menu.rs:110-120` alone, so "silent" is not the current state. `[unverified]` the total
count of 26 -- no toolchain here. `docs/ledger.md` row 101g-h claims the residue is three
deliberate shapes, and `docs/ledger.md:136` re-lists the same three, so the drift accumulated
after that batch rather than being miscounted in it.

## 4. Markdown that does not render as written

### 4.1 Unescaped pipes split table rows

`[verified]` Unique to `voxelcraft_audit`. `CLAUDE.md:149` and `docs/ledger.md:87` embed
`sky:4 | r:4 | g:4 | b:4` inside a table row, so a 3-column row renders as 5-6 cells;
`docs/ledger.md:136` has a bare `VULKAN|DX12` doing the same. Escape them (`\|`) or move the
bit layout into prose. `CLAUDE.md:367` carries the same string in an indented prose block,
where it is harmless -- do not touch it.

### 4.2 Rows detached from their table

`[verified]` `docs/ledger.md:47`, `:49`, `:51` (rows 28, 28b, 29) each sit alone between
blank lines and so render as three headerless one-row tables; `docs/roadmap.md:84` (row
`D1`) is orphaned the same way.

### 4.3 A phantom exception list, and two files disagreeing about it

`[verified]` `.gitattributes:5` names three CRLF strays, the third being
`docs/batch-11-godrays.md`, which does not exist in this tree. `docs/pitfalls.md:62` lists
only two strays. At most one can be right, and the missing file makes `.gitattributes` the
wrong one.

### 4.4 Documents citing a test file that is gone

`[verified]` `PERF.md:1301` says "the checks that matter here are five source-level guards in
`tests/pretest.rs`", and `docs/ledger.md:75` records the same file. `tests/pretest.rs` does
not exist, and neither `--no-vis-pretest` nor `SPEC_VIS_PRETEST` appears anywhere in `src/`.
`march.wgsl:97-108` states the arm "was built, measured and thrown away in batch 50. Do not
rebuild it." The substance is correctly recorded as unshipped; only the present-tense
reference to a file and a flag is stale. Nothing checks backticked paths, which is why this
survives `cargo test`.

## 5. Claims checked and rejected

1. `[rejected]` "The export tool's `IGNORE_DIRS` containing `bin` silently deleted
   `src/bin/`, and the ledger records the binaries as landed." `src/bin/` is indeed absent
   here, but the cause is not the exporter and the ledger says the opposite of what those
   reports claim it says: `docs/ledger.md:136` records that **`git log --all -- src/bin`
   shows it was never tracked on any ref**, and that HEAD carrying no `src/bin` is what made
   a batch-102 revert binary legitimate. The finding that survives is §3.2 -- the vacuous
   guard -- and it is the one `voxelcraft_audit` hedged toward. Two of three reports guessed;
   the third declined to.
2. `[rejected]` "`.gitattributes` line-ending claims cannot be checked from the dump, and
   three documents count the research PDFs differently." The first half is true of their
   artifact and moot here -- this checkout has the file, and §4.3 is the real defect. The
   second half misreads a sentence: `docs/pitfalls.md:74-75` says "Four of the five PDFs in
   there come back mixed under that test", which is a statement about a newline heuristic,
   not a competing count of `docs/research-review.md:3-7`'s seven-then-eight. `docs/research/`
   does not exist in this tree at all.
3. `[rejected]` "DS01 stores blocker distance in R16Float, about 13.84 MiB at 1920x1080,
   halving R32Float." The format is right (`src/render/shadow.rs:12`) and the number is
   right, but the number belongs to all three shadow targets at 7 B/px, not to the blocker
   map, which is 3.96 MiB. Also: `codebase_error_report` writes that same all-in figure as
   "14.6 MiB" where 14,515,200 bytes is 13.84 MiB / 14.52 MB. Two unit slips, one number,
   three names for it.
4. `[not an audit]` `From Code Audit to Autonomous Workflow` (already committed at the repo
   root) prescribes keeping `CLAUDE.md` under 200 lines with a 300-line ceiling. This file is
   667 lines / 92 KB and its rows are load-bearing -- `tests/docs.rs` validates their links
   and flag table. Trimming it would not remove §2.4's drift, it would hide the row that
   lets you see it.
5. `[unverified here]` `docs/gpu.md:214` ("`march` and `resolve` are the only passes with
   more than one pipeline") was confirmed against `SpecPipes` at `src/render/mod.rs:1730-1742`,
   which holds seven fields. The 17 broken links attributed to dump omission were not
   re-counted, since this checkout is itself upload-derived and not a clone of the working
   repository.

## 6. Suggested order of work

1. `common.wgsl:2339`/`:2135`/`:2238` -- hand the guarded direction to `march_chunk` (§1.1).
   Then re-run `validation/check_glass_transport.py`; that fixture should stop failing.
2. `shadow_ray_t` water skip (§1.2), then re-check the DS02 exactness claim.
3. `menu.rs:111` non-toggle (§1.3), and delete or wire the two dead settings.
4. `EXPERIMENTS.md:118-126` recipes and the `--no-shadow-pass` revert claims (§2.1-2.2),
   then re-measure anything that used them.
5. The §2.3 default-flag prose, §2.4's `SPACING` cluster and §2.5's VRAM row -- docs-only,
   present-tense statements only, leave the dated rows alone.
6. `tests/support/` settle helper (§3.1) and the `tests/bins.rs` early return (§3.2).
7. Escape the pipes (§4.1) and re-seat the orphaned rows (§4.2) -- lowest risk, and it makes
   the briefing readable again.

`docs/research/` is absent from this checkout, and `screenshots/` holds only its README. The
GPU-side claims in §1.1-§1.2 need a real vantage run to close, and every timing figure quoted
by the original reports remains theirs, not this file's.

## 7. What was changed on this branch

Applied, in this order, and each one reviewed as a diff rather than trusted to a script:

**The §1.1 fix, in the smallest form that works.** `march_chunk` (`common.wgsl:1381`),
`shadow_ray` (`:2106`) and `shadow_ray_t` (`:2222`) each now build a guarded direction
locally and take `inv` and `pos_dir` from it. Three deliberate choices:

- Only `inv` and `pos_dir` read the guarded vector. Hit positions, normals and `t` still
  use the untouched `rd`, so `select` returning `rd` unchanged for any ray with no
  near-zero component means those rays are **bit-identical** -- this repo's whole control
  system rests on that, and shifting a hit position by 1e-6 as a side effect would have
  cost more than the bug.
- `pos_dir` had to move with `inv`. Guarding only the reciprocal leaves the step direction
  computed from the raw zero (`rd.y > 0.0` is false) while `tn.y` becomes a large *negative*
  value that then wins the axis test, so the march crawls down a cell forever. The pair is
  consistent with `ray_dir`, which is why the primary path has never shown this.
- The guard expression is copied from `ray_dir` (`:1015-1016`) rather than invented, since
  no shader validator exists in this environment.

**Corrections to §1.1 as originally reported.** The site at `:2135` is inside `shadow_ray`,
not `trace_local`. And `cutout_march` (`:1315`) has the same raw `1.0 / rd` shape on its own
DDA; it is **not** touched here, because nothing in the reports establishes that an
axis-aligned ray reaches it, and a speculative guard in a hot loop is a cost without a
demonstrated bug.

**Left deliberately alone.** `menu.rs:111` (`Setting::ShadowPass=>p.shadow_pass=true`):
flipping it to a toggle would create a control that reports success and is then clobbered by
`settings.rs:167`, which forces the value on every decode -- a worse trap than the dead arm,
and `tests/split_passes.rs:34` already pins `restore_defaults()` leaving `shadow_pass` true.
The right fix is deleting `IsolateGlass`/`ShadowPass` from the enum, `experimental()`,
`adjust()` and the settings round-trip, which crosses the `version=2` key gate at
`split_passes.rs:24` and so needs a compiler to attempt. Same reason `ENTRY_POINTS` stays at
11 entries despite §2.7: widening a public const array is exactly the edit that fails to
build, and it feeds a `shaderstats` binary that does not exist in this tree.

**Docs and comments:** every present-tense item in §2 and §3.4 -- the seven-off claim, the
revert column, the A/B recipes, the R8/7-by-per-pixel shadow layout in `EXPERIMENTS.md`, the
Render lab row, twelve stale code comments in `probe.rs`, `stream.rs`, `mod.rs`, `timer.rs`
and `common.wgsl`, the `--water-mottle` struct doc, comment and help text, `docs/gpu.md`'s
pipeline count, the `--probe-sun` rows, and `docs/water.md`'s control table. §4.1's three
pipe breaks are escaped; §4.2's detached rows are **not** re-seated, since moving table rows
around in `ledger.md` risks the one thing `tests/docs.rs` does check.

**Two new facts, found while editing rather than auditing.** `.gitattributes` declares every
`.wgsl` LF; this tree is 3261/3261 CRLF, and `docs/` is CRLF where the file says LF -- so
§4.3's problem is wider than a phantom third exception, and given `* -text` the bytes are
what a clone gets. Second: the prose here is hard-wrapped, which is why any single-line
substitution in a wrapped paragraph has to be self-contained; four of my first-pass
replacements read correctly alone and broke the next line, and were rewritten.

**Verification actually performed here**, none of it by compiler: per-file line-ending census
before and after (no file became mixed; `common.wgsl` gained exactly 12 lines), brace parity
against `HEAD` (259/259 unchanged), paren parity (+18 open, +18 close, exactly the three
guards), each construct confirmed to exist verbatim elsewhere in the same module, `rd_dda`
defined before every use in all three functions, and every edit anchored on a substring that
occurs exactly once -- five anchors failed that test and were skipped then repaired rather
than force-applied.

**Still owed, and not claimable from this environment:** `cargo check`, `cargo clippy`,
`cargo test`, the `bitexact` sweep, and `validation/check_glass_transport.py`, which is the
run that decides whether §1.1 is fixed. Expect it to stop failing and expect no pixel change
anywhere else; if `bitexact` moves on a vantage that has no axis-aligned ray, the guard is
wrong and should be reverted rather than tuned.
