
# Pitfalls: the errors this tree actually produces

**Read this before you type a command, not before you plan a batch.** Everything here fires at
the moment of an action -- a shell invocation, a file edit, a restore -- and every entry cost at
least one session a turn to rediscover. That is why it is a checklist keyed on the *trigger*
and not the prose paragraph it was until batch 25: the information was in
[`../CLAUDE.md`](../CLAUDE.md) all along, and batch 25 walked into two of these anyway, because
prose is not indexed by the thing you are about to do.

The neighbouring files: [`lessons.md`](lessons.md) is what to think about when *planning* a
batch, [`errors.md`](errors.md) is the defects that are measured, priced and
deliberately not chased, and each `docs/batch-N` file holds its own batch's misses.

## Shell and editing

| If you are about to... | ...then |
|---|---|
| write a heredoc | `cat <<'EOF'` **fails here** (`unexpected EOF`). Use `python - <<'PYEOF'`. |
| write a *long* heredoc | Over about a hundred lines it fails outright whatever is in it. Write long prose with the file tool and splice it in with a short script. |
| put a backslash in a heredoc | **Backslashes inside `python - <<'PYEOF'` are mangled** -- a literal `\n` comes back reordered. Build them from `chr(92)`, or anchor on a substring that has none. |
| pipe a long `cargo` run anywhere | **Never pipe it through `tail`.** It buffers and you get nothing when it finishes. Redirect to a file and `grep` that. |
| pass a path to Python | Git Bash's `/tmp` and Windows Python's `/tmp` are **different directories**. Pass Windows paths. |
| anchor a string replacement in `CLAUDE.md`, `PERF.md` or `README.md` | They are full of em dashes. Anchor on **ASCII** substrings or the match silently fails. |
| survey line endings | **Do not use `grep -c $'\r'`.** In Git Bash that expands to the empty pattern, which matches every line of every file, so the whole tree reports as CRLF including the pure-LF majority. Batch 22b believed it for three turns and wrote it up as a finding. Count bytes in Python. |
| **append a line to `src/lib.rs`, `src/stats.rs`, `Cargo.toml`, `CLAUDE.md`, `PERF.md` or `README.md`** | **Those six are CRLF and a Python replacement string is not.** Batch 57 added one `pub mod` line to `src/lib.rs` with `newline=''` set correctly and still wrote a bare LF into it, because `newline=''` protects the text you *read* and not the text you *build* -- the row below says so and the batch walked into it anyway, which is the **sixth** line-ending incident here and the second of exactly this shape. The fix that works is structural rather than careful: read the file as bytes first, and if it is pure CRLF, normalise the whole thing after the edit and count the pairs. |
| put a newline *inside* a replacement string | A Python `\n` in the replacement writes a **bare LF into a CRLF file** -- `newline=''` protects the text you read, not the text you build. Normalise the whole file after any multi-line substitution and **count the bytes**. Batch 28 did this to `CLAUDE.md` and caught it only on the count: the **fourth** line-ending incident here. Batch 32 is the **fifth** and is the inverse -- a sentence *about* `\r\n` put a real one into an LF document, because a heredoc mangles the backslash and leaves a control character where the text was. Build both from `chr(92)`, and count the pairs after every write, not at the end of the batch. |
| restore a file you moved aside | `shutil.move` and `copy2` **put the old mtime back, so cargo does not rebuild**. Call `os.utime(p, None)` after any restore. |
| **run any `harness` subcommand after editing the renderer** | **`cargo run --bin harness` does not rebuild `voxelcraft.exe`, and `voxelcraft.exe` is the binary the harness spawns.** Cargo builds the `harness` bin and the library; the `voxelcraft` bin target is not in that graph, so the fixture happily measures whatever renderer was left in `target/release/` by the last command that *did* build one -- and `cargo test --release --test X`, `cargo clippy` and `cargo bench` are all commands that do not. **Every row comes back plausible and internally consistent**, which is this file's recurring failure mode. Batch 57 changed one ramp in `probe.rs`, re-ran `bitexact --only cave` through `cargo run --bin harness`, read the pixel count as **unchanged to the unit**, and concluded the change had done nothing; the same sweep after a real build read **30,300 instead of 369,260**. Run `cargo build --release` -- which builds every bin -- before any fixture command, and if a count comes back *identical* to the last one, suspect this before suspecting the code. |
| **rank a change to `march`** | **Count it before you time it.** `harness march` reports the marcher's DDA steps as integers, so it is immune to every caveat in `PERF.md` and to this table's two bench rows -- a hot laptop, a competing process and a power state all move a millisecond and none of them can move a count. Use `bench` to confirm a direction the count already established, not to discover one. |
| **bench anything at all** | **Check that no `voxelcraft.exe` is already running.** The user plays this game, and a window left open competes for the GPU: the post-batch-47 frame map first read `resolve` **17.83 ms at `default` against a true 6.09, and 27.88 at `coastline` against 9.81** -- a 3x inflation, well past the 1.8x this tree attributes to power state, and every row internally consistent so nothing in the output says it is wrong. `tasklist \| grep -i voxel` before a sweep; two processes means the numbers are junk. A *paired* bench survives this better than a standing measurement, because the contention hits both sides -- but a frame map is absolute by construction and cannot be saved. |
| **bench late in a long session** | **Read the standard error against the same vantage's earlier runs, not against zero.** The fixture is built around a **+0.18 ms (2%)** warm-up ramp, measured over sixteen consecutive runs. Under sustained load a laptop leaves that regime: batch 50 benched `underwater` after ~90 minutes of continuous sweeps and `resolve`'s A side climbed **3.51 -> 4.18 over seven rounds, +19%** -- ten times the documented ramp -- with the standard error **0.021 against the 0.004-0.008 the same vantage gave two hours earlier**. The user's fan monitor read **CPU 94C, GPU 80C** at that moment, which is how it was caught; nothing in the output says it. **Pairing and alternation still cancel it**, so the verdict survives -- what is lost is *resolution*, and a batch that quotes "indistinguishable from zero" off a tripled standard error is claiming much less than it sounds like. Quote the detection threshold (about 3 se) rather than the word. Two ways out: let the machine idle, or ask a question a **count** can answer -- batch 50's mechanism came from tallying writes, which no thermal state can move. |
| **render a vantage by hand** | **Take the arguments from `harness list`, never from memory.** Batch 53 retyped four of them while measuring a traversal change -- `terraces` as `--time 0.20 --cam-pitch -20 --cam-yaw 8`, `canopy` as `--cam-height 2 --cam-pitch 6 --cam-yaw 96` -- rendered four cameras nobody had asked for, and read the difference as a defect in the feature under test. **Every one of those runs exited 0 and wrote a plausible frame.** `harness march`, `bitexact --only`, `compare --vantage` and `capture --only` all take a *name*; there is no reason to ever type the arguments. |
| drive a sweep from `harness-base.exe` in the repo root | **Pass `--exe target/release/voxelcraft.exe` explicitly.** `default_exe()` is *`voxelcraft` beside this binary*, which is `target/release/` when cargo runs it and the **repo root** when the copied fixture does -- where no `voxelcraft.exe` exists. `capture` and `validate` work because cargo runs them; a hand-driven `bench` or `bitexact` fails per vantage. Batch 44 lost an eleven-minute sweep to it and the failure prints once per row rather than once. |
| A/B a control the *old* binary cannot parse | **`--extra` applies to both sides.** Side B is a revert binary that predates the flag, so it hits `config::UNKNOWN_ARG_MARK` and the row is refused -- correctly. Use `--extra "--no-thing" --extra-b ""`: side B's set is empty and explicit. Batch 45 hit this on the control-purity check, which is the one command a new control most needs. |
| **read a `bench` delta** | **It is `B - A`, not `A - B`** -- `bench.rs` computes `d = y - x` with `x` from side A. So with the feature on side A, a **negative** delta is what the feature **costs**, which is the opposite of what the sign reads like. Batch 55 got it backwards off the round-by-round lines, wrote a paragraph concluding the tap made `resolve` *faster*, and caught it only by opening `bench.rs`. Nothing in the output says which way round it is, every row is internally consistent either way, and the verdict line inherits the same sign -- so a whole batch can be written up inverted. **Check the sign against a row whose answer you already know**, or read the four lines of `Paired::cell`. |
| parse `bitexact`'s table with a regex | **The pixel count is bolded only when it is zero.** `\*\*?` requires at least one asterisk and so silently drops every row that *moved* -- which in a sweep looking for movement is the failure mode that looks like a pass. `\*{0,2}` on both sides, and assert the row count. |
| run a sweep through `harness-base.exe` after editing `src/harness/` | **Re-copy it first.** The fixture's `IMPLIES` and `CHECKS` tables are compiled *into* that binary, so a copy taken at the top of the session checks the table as it was then. Batch 41 added two claims, ran `implies` through a stale copy, and got **"26 claims, every one as the control table says"** -- a clean pass that had not looked at either new claim. It announces itself only by the count, which is why the verdict line prints one. |
| read a research `.docx` the user hands over | **`pandoc` is not installed on this machine**, so the docx skill's read path does not work here. Unzip it in Python and walk `word/document.xml`; tables live in `w:tbl` and are lost by a naive `w:t` scrape. |
| read a *number* out of a research `.docx` | **Every formula and most table cells are embedded PNGs** -- three document sets running, and the set that was explicitly asked for text did it anyway. Map `a:blip/@r:embed` through `word/_rels/document.xml.rels` to `word/media/`, and **alpha-composite each PNG onto white before reading it**: they are black glyphs on a transparent ground, so a plain `.convert('RGB')` gives a solid black box and reads as a redaction. |

### Writing a file from Python will change its line endings

**Always `open(p, 'w', encoding='utf-8', newline='')`** -- for LF files exactly as much as for
CRLF ones. Python text mode translates `\n` to `\r\n` on Windows, so a plain `open(p, 'w')`
silently converts an LF file to CRLF.

This entry replaces a sentence that had it wrong. Until batch 25 this file's ancestor said a
`python - <<'PYEOF'` heredoc "is the only reliable way to edit the CRLF files without
normalising them", which credits the heredoc for a property it does not have: a heredoc without
`newline=''` normalises exactly the same as anything else. Batch 25 got six edits right by habit
and two wrong, turning two LF documents into CRLF -- the **third** line-ending incident in this
tree.

### The line-ending map, which is real and mixed

`.gitattributes` is `* -text` **and has to stay**: Git for Windows defaults to
`core.autocrlf=true`, which would normalise the LF half of this tree to CRLF on the next
checkout and rewrite most of `src/`.

- **CRLF (6)**: `Cargo.toml`, `CLAUDE.md`, `PERF.md`, `README.md`, and
  two strays -- `src/lib.rs`, `src/stats.rs`.
- **LF (86)**: every `.wgsl`, and every `.rs` and `docs/` file apart from those two. **86 as of
  batch 57, 85 at 54, 84 at 53, 83 at 52, 82 at 43, 81 at 38b, 76 before that** -- each taken by counting rather than by
  quoting the line above it, the way this section says to. Batches keep adding documents and
  tests, so the number moves and the *list* is what is stable.

**This list said `docs/sky.md` was CRLF until batch 30, and it is LF with no batch having
converted it** -- so the entry was either wrong when written or the file was normalised by one of
the four line-ending incidents above and only the code was ever put back. The counts moved too,
from *8 against 91*: that was a tree with `DO-NOT-READ-CLAUDE.md` and twenty-one batch documents
still in it. **Count them rather than quoting this**, the way `ledger.md` says to count its own
rows: `git ls-files`, read each file as bytes, and compare the number of CRLF pairs against the
number of newlines. **Exclude `docs/research/` before you believe the total.** Four of the five
PDFs in there come back *mixed* under that test and announce themselves as binary; the fifth,
`Voxel Radiance Cascades Architecture.pdf`, holds `\n` bytes and no `\r\n` and so counts as a
clean LF **text** file, which is how a correct 76 reads as 77 and sends a session looking for a
file no batch added. Do not open them to find out which is which, which is the whole point of
that rule.

**`screenshots/` is the same trap and there are fifty-seven of them.** Every tracked `.png`
comes back *mixed* -- a PNG's compressed bytes hold both bare and paired newline characters
often enough that the ratio is neither 0 nor 1 -- so a survey that does not filter on
extension reports a tree full of mixed files and buries the six that matter. **Count text
files only**, by extension, and read *mixed* as evidence that a file is binary rather than
as a finding about it.

**Check the file before anchoring on a newline rather than trusting the rule**, and check it
again after writing.

## wgpu and WGSL

**These moved out of `CLAUDE.md`'s invariants list in batch 28.** They were never invariants in
that file's sense -- nothing here is a decision voxelcraft made, and every one of them fires the
same way in any project against these APIs, including one that *does* have a mesher. They are
here because they fire at the moment of an action, which is what this file is keyed on.

| If you are about to... | ...then |
|---|---|
| write a shader that dispatches indirectly from a buffer it also binds | **A buffer cannot be both a storage binding and an indirect dispatch argument in the same pass.** Shaders write `indirect_storage`; it is copied to `indirect` between passes. |
| bind a texture for writing and sampling at once | **A texture cannot be a write storage binding and a sampled binding of the same bind group.** Same class of error: wgpu merges every resource of a bind group into the pass's usage scope. It is why `taa` gets its own bind groups. |
| shift a `u64` in WGSL | **The shift amount must be `u32` even on a `u64` value** -- `key >> 40u`, never `40lu`. |
| write a `select` whose first two arguments are comparisons | **WGSL reads `f(a < b, c > d, ...)` as a template argument list**, so this is a *parse* error, not a type error. Write the branches as `if`s. |
| create a surface or an adapter | **The instance must outlive the surface, and the adapter must come from the same instance.** `App` holds `_instance` for exactly this reason. |
| dispatch more than 65535 workgroups | **Compute dispatch is capped there**, so `march` strides over pairs in a loop. |
| name a WGSL parameter or variable | **The reserved-word list is far longer than the keyword list**, and `class` is on it -- a plain-looking name is a *parsing* error at `create_shader_module`. Batch 36 lost a run to `fn permute_uv(uv, class, h)`. |
| request any `EXPERIMENTAL_` wgpu feature | **The device request is refused unless you also pass `experimental_features: unsafe { wgpu::ExperimentalFeatures::enabled() }`.** The error says *"experimental features are not enabled"* and names the features, which reads like the adapter lacking them when the adapter reports them `true`. Batch 69. |
| write a ray query or a cooperative matrix in WGSL | **Each needs its own `enable` directive at the top of the module**, before any declaration: `enable wgpu_ray_query;` and `enable wgpu_cooperative_matrix;` (the latter also needs `enable f16;` for an `f16` matrix). naga names the missing one in the error, so this costs one run rather than a session. |
| call `coopStore` | **The matrix is the *first* argument and the pointer the second** -- `coopStore(mat, &buf[0], stride)`. Reversed, naga reports *"Invalid target type for a cooperative store"*, which reads as a problem with the pointer. `coopLoad` is the other way round and takes the matrix as a template parameter: `coopLoad<coop_mat16x16<f16, A>>(&buf[0], stride)`. |
| run `cargo test` after editing a shader | **It does not compile the shader.** Nothing in the suite creates a device, so `shader_source()` is only ever inspected as text: a WGSL syntax error passes the whole suite green and fails at the first `--screenshot`. **Render one frame before you believe a shader edit.** |

## Lints

**Clippy is not clean -- an external audit counted 26 warnings -- and every `allow` carries a
reason at its own site. Do not run `clippy --fix` blind on this tree** -- it is full of
formulations that are load-bearing rather than incidental. (This row also predates the tree
going under git at batch 27: the diff is reviewable now, and the rest of the warning stands.)

Batch 22 ran it for the first time: 37 warnings, 23 in the library and 14 in the tests, with
**no dead code and no bugs** among them -- all style. Seventeen rewrites were kept; the other
twenty are suppressed, two of them after being applied and put back. Three refusals are the
ones worth knowing about, because each would have cost real meaning:

- `encode_leaf`'s `i` is a **bit position as well as an index**; iterating the slice throws the
  bit position away.
- The snow and tree lines read `h - 1 >= line` because **`h - 1` is the top solid block** and
  `h` is the air above it. Clippy's `h > line` is the same integers and says nothing about
  which voxel.
- Four test files hold **deliberate transliterations** of `src/`, whose whole value is reading
  the same as the original.

## Known warts

- **On an sRGB *surface* the UI pass draws after the blit**, so its authored colours get
  hardware-encoded and read brighter than in a `--screenshot`. Not a regression. Fixing it
  means plumbing the encode flag into `ui.wgsl`.
- **`harness`'s own argument parser silently ignores unknown arguments** (`Opts::parse` ends in
  `_ => {}`), which is the defect batch 22b called "the one failure mode indistinguishable from
  success" and fixed for the *renderer* via `config::UNKNOWN_ARG_MARK`. The guard protects the
  inner call and not the outer one. Batch 25 tried to produce a false pass from it and could
  not -- a dropped `--exe-b` errors properly and a misspelled `--vantage` falls through to all
  fourteen, which announces itself by taking ten minutes -- so this is an inconsistency worth
  one line of fix rather than an alarm.
- **`compare` takes two `.png` paths** where every sibling subcommand takes `--vantage`, and
  **`bitexact` exits non-zero whenever images differ**, which is right for a control check and
  awkward when using it to measure how far a feature moves.

