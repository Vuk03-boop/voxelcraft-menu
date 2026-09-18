# Persistence

The edit journal: what the player changed, written down and replayed, at every level of detail
-- and, since batch 32, where the player was standing when they stopped changing it.
**Batches 29, 30, 32, 33 and 34**, the goal the roadmap chose after batch 20's finished -- see
[`roadmap.md`](roadmap.md). 29 built the journal and gave it to LOD 0; **30 gave it to the
coarse levels**, which is what stopped a structure vanishing as the player walked away from it;
**32 put the player in the header**; **33 made it a world on disk that grows as you build**,
so a session killed rather than closed keeps what it built; **34 put the bar's contents in it**,
so a world comes back with the bar it was left with. What is still missing is in
[`errors.md`](errors.md); the flags are in `--help` and the file format is below.

---

## What is persisted, and what is not

**Only the deltas.** Terrain is a pure function of the seed, so worldgen plus the deltas *is*
the world; writing chunks out would be writing down something the generator already knows. A
journal of the hut `--demo-edits` builds is **6,368 bytes** -- 393 edits at 16 bytes each, plus
a 24-byte header, a 32-byte player block and a 24-byte bar block -- against the 888 chunks that
vantage generates. **This said 6,312 until batch 34 and had been the wrong number since 32**:
it was written when the header was the only thing in front of the records, and the two optional
blocks added since are exactly the 56 bytes it was missing.

Not persisted, each for its own reason:

- **Coarse terrain.** It is still resampled noise at stride 2^n and should be: one code path,
  only the stride differing, so the altitude rules agree across a transition by construction.
  What is persisted at a coarse level is the **deltas**, aggregated -- the section below.
- **Stacks, and the time of day.** Batch 32 landed the selected slot and **34 the bar's
  contents**, so what the player is holding survives a reload. Neither is a *stack*: nothing can
  run out, and a count per slot is the other half of the roadmap's inventory item -- a different
  feature, and the one that would make an edit refusable for the first time. The bar block's two
  spare `u16` are where it would go.
- **The player's position, continuously.** Batch 33 appends every edit as it lands, so the
  *deltas* survive a kill; the player block is still written by the exit save alone. A killed
  session therefore comes back where the last clean exit left it, which is a coherent answer
  and not the exact one -- the reason it is a separate question is in the section below.
- **`--bench-terrain`.** It measures the generator over a fabricated grid of chunk keys and
  builds its world without a `Streamer`, so it replays nothing. The other three modes all do.

---

## Where the two halves live

**Recording is one site**: the point in `World::set_block` where a write is known to be real --
past the height bound, past the residency check, and past *both* redundant-write returns. The
two callers that edit (`App::handle_edits` and `scene::build_demo_structure`) therefore need no
code at all, and a third cannot be added without one. That is the argument `scene.rs` makes
about `flags_from`, and it pays here immediately: **`--demo-edits` produces the control's
journal for free**, with no line of code that exists only for the test.

**Replay is in `stream::build`**, between `generate` and `build_local`, on the rayon worker --
not through `set_block` after interning. Three reasons, in the order they matter:

1. **Lighting falls out for free.** `light::compute` runs on the edited dense array, so a
   reloaded chunk needs no relight pass and cannot disagree with a live-edited one about sky
   access.
2. The main thread pays nothing when a chunk reappears.
3. `ChunkRecord::regions` grows one entry per non-uniform-leaf edit and is freed only on
   unload; a `set_block` replay would pay that again on every reload.

One `Arc<EditJournal>` is shared by the `World` that records into it and the `Streamer` whose
workers replay from it -- `World::attach_journal` and `ChunkManager::attach_journal`, called
together at all three construction sites. It is always present and empty by default rather than
an `Option`, so there is one code path and "persistence is off" is an empty map.

## The edit already evaporated mid-session, and that is the half no capture can see

`ChunkManager::update` unloads a chunk `unload_grace_frames` (240) after nothing needs it, and
the chunk regenerates from noise when the player walks back. **Before this batch that took the
edits with it.** Replay at generation fixes it with no flag: the default build now does this,
which is a behaviour change the fourteen-vantage control cannot show, because a settle-and-
capture never unloads anything. `edits_survive_a_chunk_unload` in `tests/edit_journal.rs` is
the only thing that pins it.

---

## The file

Fixed layout, little-endian, `bytemuck` in and out -- no serde, no new dependency.

| field | why |
|---|---|
| `magic: u32` = `VXJ1`, `version: u32` | a foreign or half-written file has to fail loudly rather than replay garbage |
| `seed: i32`, `sea_level: i32` | **the load-bearing guard.** A journal replayed into another world lands every edit on terrain that is not there -- a hut in mid-air -- and **every capture of it still looks like a capture**. Refusing the load is the only place that can be caught |
| `count: u32`, `flags: u32` | `flags` was the spare word. Bit 0 says a player block follows (batch 32); **bit 1 says the append log wrote this file, so its tail may hold whole records `count` does not claim** (batch 33); bit 2 says a bar block follows the player one (batch 34). Twenty-two bits still spare. **Every optional block is a bit, and that is what makes three versions one reader** -- a file's shape is read off this word, never off the version number |
| the player block, when `flags` says so | 32 bytes: feet position, yaw, pitch, hotbar slot, flying. Between the header and the records, so both stay 8-byte aligned. Batch 32 |
| the bar block, when `flags` says so | 24 bytes: ten block ids and two spare `u16`. After the player block, and its size is what keeps the records 8-byte aligned in **all four** combinations of present blocks -- which is why the assert is on the sums and not on this struct. Batch 34 |
| `count` x `{ x, y, z: i32, id: u16, pad: u16 }` | 16 bytes. `y` is **internal** world Y, the coordinate `set_block` takes, not the one the stats overlay prints |

In memory it is `chunk -> dense index -> id`, grouped the way replay reads it, and
**last-write-wins inside a chunk**: a journal with no ordering to preserve needs none to be
correct, so replay does not depend on the map's iteration order. Save **sorts by (x, z, y)**, so
the same world writes the same bytes -- an artifact that differed run to run would be one nothing
could diff, which is the argument the capture manifest's hashes make.

**Since batch 33 there are two writers and the file's order means different things to each.**
`save` compacts: one record per position, sorted, and the order is there only to make the bytes
reproducible. The append log is a *log*: place, break, place again is three records at one
position, in the order they happened, and **the loader's last-write-wins is that order**. Both
replay to the same blocks -- `the_log_and_the_exit_save_agree_about_the_world` is the claim --
and the compacted file is the smaller one.

A missing `--load-edits` file is an error, not an empty journal, for the same reason the seed
guard exists: a mistyped path would otherwise render an unedited world and say nothing. **A
missing `--edits` file is a new world**, which is the same argument reaching the opposite
answer, because a world nobody has played is the one case where the file cannot be there.

---

## Controls

| flag | does |
|---|---|
| `--load-edits PATH` | replay a journal after worldgen. Absent is a world that is exactly what the seed says it is, which is every capture taken before this batch |
| `--save-edits PATH` | write the journal -- loaded entries and this session's alike -- when the mode ends |
| `--resume` | start the player where the loaded journal says, instead of at the vantage the camera flags describe. Needs `--load-edits`, and a journal that carries a player. Redundant in the window, which already does it. Batch 32 |
| `--hotbar N` | the slot the player starts on, 0-9. Ships at **4**, the slot `--hud` has always drawn. `--resume` overrides it, the way a loaded journal overrides everything else about the player. Batch 32 |
| `--edits PATH` | **a world on disk**: load it if it is there, append every edit as it lands, write it back compacted when the mode ends. A path with no file at it is a new world, said out loud. Refused alongside either flag above. Batch 33 |
| `--no-exit-save` | end without the exit save, which is what a kill looks like to the file. The only way a fixture can observe the append log. Batch 33 |
| `--bar A,B,...` | the ten block ids in the bar, refused as a whole if any one of them is wrong. What lets a capture reach a bar no default can, the way `--hotbar` does for the selection -- and it is **refused against a journal that already carries one** rather than losing silently to it. Batch 34 |

Two flags rather than one because **each half has to be separately runnable**: the A/B below is
one binary rendering `--demo-edits --save-edits f` against `--load-edits f`, and a single flag
that did both could only ever compare a build against itself. That is a *measurement* pair, and
it is why a missing file is an error there.

**`--edits` is the world-shaped flag, and it is not those two spelled shorter.** Playing a
persistent world used to be both at one path:

```bash
cargo run --release -- --load-edits w.journal --save-edits w.journal   # and it must already exist
```

```bash
cargo run --release -- --edits w.journal                              # and a kill costs nothing
```

The append log hangs off `--edits` and **not** off `--save-edits`, which is deliberate: every
A/B ever taken through those two flags still writes the byte-for-byte file it wrote in batch 29.

**Under `--load-edits`/`--save-edits` a session killed rather than closed still loses
everything**, and that is now a property of those two flags rather than of the engine:
`save_if_asked` runs once, in `ApplicationHandler::exiting`, and nothing else writes. Under
`--edits` the deltas are safe the moment they land, and **what a kill still costs is the
player's position** -- the file keeps whoever the last clean exit stamped. The two halves come
apart here for a reason worth knowing: an edit is discrete and a position is continuous, so
appending fixes the first exactly and cannot fix the second at all. That residual is the one
entry batch 33 left in [`errors.md`](errors.md).

The windowed half is the one thing here no fixture reaches: `ApplicationHandler::exiting` calls
the same `journal::save_if_asked` the headless modes call, and *that* function is exercised
fourteen times by the sweep below -- what is unverified is the one line binding it to the event
loop. `app.rs` says the same thing about itself for the same reason: no capture opens a window.

---

## Batch 34: the bar in the file

Batch 32 persisted *which slot* was selected. This persists *what is in the slots*, so a world
comes back with the bar it was left with rather than with the one the build was compiled with.

`block::HOTBAR` was a `const [BlockId; 10]` that four files read directly. It is now the
**default** bar: `Player::bar` holds the live one, the journal carries one, `--bar` authors one
and middle-click fills a slot from the world. Nothing about the picture changes for a player
who never picks a block, which is the whole of why the existing capture corpus is untouched.

### A second presence bit, not a wider player block

`FLAG_BAR` is bit 2 of the header's `flags`, and it says a 24-byte bar block follows the
player block. The alternative -- five more words on the end of `PlayerState` -- fails for one
specific reason rather than on taste: **a version-2 file has a player and no bar**, so the
player block's size would depend on the version number and reading it would become a branch on
that number. That is the exact thing `VERSION`'s comment says the `flags` word exists to avoid,
and batch 32 spent the word to avoid it once already.

So the claim batch 32 made about two versions -- *one reader, no branch on the number* -- is
now made about three, and this is the first time it has been tested by a file the reader had
never seen before. `a_file_with_no_bar_block_loads_and_means_the_default_bar` is that test.

**24 bytes, and the two spare `u16` are load-bearing.** Ten `u16` slots is 20, and the records
behind the block are read in place by `bytemuck`, so every combination of optional blocks has
to leave them 8-byte aligned. A size assert on the bar block alone cannot see that: the
quantity is the sum, and all four sums are asserted at the top of `journal.rs`. The spare pair
is also the next feature's room -- a count per slot is a `u16` per slot, which is what the
inventory item in [`roadmap.md`](roadmap.md) would want and what `FLAG_BAR` being a *bit*
leaves space for.

The other half of "one expression, many readers" grew with it: `player_bytes` became
`extras_bytes`, a **sum over present blocks**, and it has three readers -- the loader splitting
the bytes, the append log seeking past them, and the compacting save sizing its buffer. A file
whose readers disagree about where the records start replays every edit some bytes shifted and
still looks like a journal.

### The boundary is the mode's, and a bar is not the mode's

Batch 32 drew a line: a loaded *camera* needs `--resume` to take effect, because the fourteen
named vantages **are** the camera flags, and a journal that silently moved one would turn every
vantage into "wherever the file says". `scene::start_player`'s own comment goes further and
warns specifically against splitting that per field -- letting the file pick the hotbar while
the camera flags keep the camera.

**The bar is outside that line, and the reason is not that the rule was inconvenient.** The
rule is about the fixture: no vantage names a bar, so there is nothing for a loaded bar to
overrule. What makes it safe rather than merely arguable is arithmetic -- a player who has
never picked a block carries `block::HOTBAR`, which is what the bar would have been anyway, so
**every capture ever taken renders identically either way**. The `bitexact` row below is what
turns that from an argument into a measurement.

`--resume` therefore still gates position, angles and slot together, and the bar is restored
unconditionally by `journal::bar_for`, which both the window and the capture path call.

### Four blocks a bar may not hold, and each one keeps an argument

`block::bar_slot_refusal` is the rule, and it is one function because there are three callers:
the file's edge, `--bar`, and picking. Until this batch the only sources of a block id were the
generator and a `const` array, and three places in the tree reasoned about that.

| refused | what it keeps true |
|---|---|
| `TALL_GRASS` | **the hard one.** `SPEC_FOLIAGE` compiles the marcher *unable* to see a tuft in a run the generator gave none. The `const` assert beside `render::FLAG_FOLIAGE` exists to stop that arriving quietly and can only ever speak for the compile-time array -- so a bar from a file is precisely the case it cannot see, and a placed tuft would be invisible rather than wrong |
| `WATER` | `scene::flags_from`'s argument: a zeroed sea level floods nothing, so `FLAG_WAVES` may assume a run with no sea can never acquire a water pixel later |
| `AIR` | not a hazard but a category error -- a slot holds the block you place, and placing air is what breaking one already is |
| anything past `BLOCK_COUNT` | a panic in `block::def` three call sites later, which is `PlayerState::check`'s argument about the hotbar slot, one field over |

**Picking is where that list earns its keep most**, and it is the one caller with nothing to do
with files: the world is full of tall grass and water, and the crosshair can be pointed at any
of it. `picking_refuses_exactly_what_a_file_is_refused` holds the two together.

`--bar` is **refused as a whole rather than clamped**, which is the one place it deliberately
differs from `--hotbar`. A slot *number* out of range has an obvious nearest legal answer and a
human at the other end who can see which cell they got; a block *id* has none -- the nearest
legal id is a different block, and a bar quietly holding sand where the command line said tall
grass is a capture that looks exactly like the capture that was asked for.

### What batch 34 measured

#### The bar, end to end, at pixel resolution

The bar is visible in exactly one place, so this is a `--hud` capture at 1280x720. One binary,
three runs: author a bar into a new world, reload that world, and the same camera with no
journal at all.

| capture | arguments | pixels differing from *live* | sha256 |
|---|---|---|---|
| *live* | `--hud --edits w34.journal --bar 14,15,5,12,17,16,19,1,2,3` | -- | `0ab602ea...4712` |
| *reloaded* | `--hud --edits w34.journal` | **0 of 921,600** | `0ab602ea...4712` |
| *control* | `--hud` | **1,604**, max 241 | `aba3fccd...50cf1` |

**The pair is the point and the second row alone would be vacuous** -- that is the failure
[`errors.md`](errors.md) names, and batch 32's first attempt at this same shape walked into it.
The control row is what says the 0 is a bar that travelled through a file rather than a bar
nothing ever changed: 1,604 pixels is the ten cell icons and nothing else in the frame.

The file is **80 bytes**: 24 header + 32 player + 24 bar + no records. A world with a bar and
nothing built in it is the smallest thing this format can say, and the arithmetic is legible in
the size.

#### What a pre-34 binary does with it

```
w34.journal: journal version 3, this build reads 1..=2
```

That sentence is the **entire** reason batch 32 bumped a version number it did not otherwise
need, and this is the first batch to collect on it: without the bump, `voxelcraft-pre34.exe`
would have failed on a byte count and blamed the edit records.

#### The control

`harness bitexact --exe-b voxelcraft-pre34.exe`: **fourteen vantages identical**. Nothing in
the fixture names a bar, no journal in it carries one, and a bar nobody set is
`block::HOTBAR` -- so this batch is invisible to every capture that predates it, by
construction rather than by luck.

#### The tests, and the negative test of the tests

Six, in `tests/edit_journal.rs`, and each positive claim is in a pair:

- `the_bar_round_trips_through_the_file` -- the bar comes back, **and** the file is 24 bytes
  longer with `FLAG_BAR` set and the records still where the reader thinks. Read from the
  file's length, not from the loader that just parsed it.
- `a_file_with_no_bar_block_loads_and_means_the_default_bar` -- the version-2 layout, built by
  hand because no file in the tree is older than the tree, and "the file does not say" resolves
  to `block::HOTBAR`.
- `a_bar_block_the_build_cannot_honour_is_refused` -- all four refusals, **and** the same slot
  with a legal block loading, which is what says the rule is about blocks and not about slot 3.
- `the_bar_flag_is_refused_as_a_whole_or_not_at_all` -- seven malformed `--bar` values and one
  good one that has to reach the journal.
- `the_bar_flag_and_a_world_that_already_has_a_bar_is_refused` -- **and** the same world
  without the flag loading its own bar, so the refusal is about the clash and not the file.
- `picking_refuses_exactly_what_a_file_is_refused`.

`records_in`, the helper that reads the record count out of a file's length, is a deliberate
transliteration of `extras_bytes` rather than a call to it -- the same bargain four other test
files here strike. **This batch is what that is worth**: the helper said `24 + player`, the bar
block made it wrong by a record and a half, and it came back as a failing assert rather than as
a quietly different number.

### What it does not do, and the residual it shares

**Nothing runs out.** This is the file-format half of the roadmap's inventory item; a count per
slot is the other half and a different feature, and the two spare `u16` in the bar block are
where it would go.

**A killed session loses the bar**, exactly as it loses the player's position and for exactly
the same reason: both are written by the exit save alone, while `--edits` appends each *edit*
as it lands. An edit is discrete and a bar is a small fixed-size block, so unlike the position
this one *could* be stamped in place -- and it sits at a fixed offset behind a block that is
already fixed-size, which means the roadmap's stamping batch gets both for one decision about
*when*. That is one [`errors.md`](errors.md) entry now covering two fields.

---

## Batch 33: a world on disk, and a file that grows as you build

Batches 29 to 32 wrote the journal **once, at the end of the mode**. Everything they built was
correct and none of it was safe: a crash, a kill, a closed laptop threw the whole session away,
and the only way to have a world at all was to name it twice on the command line and hope the
file already existed. Batch 33 is the flag a player would have expected all along --

```bash
cargo run --release -- --edits w.journal
```

-- plus the property that makes it worth having: **the file on disk is the world at every
instant, not only after a clean exit.**

### The append log, whose whole design is the order of three writes

One accepted edit is one 16-byte record on the end of the file. What makes that safe is not the
appending, which is obvious, but **the order**, because every step has to leave a file the
reader can still make sense of:

1. **`FLAG_APPEND` goes in before the first record**, not after it. A file carrying an
   uncounted record *without* that bit is one `load` has to refuse -- it is byte-for-byte what
   a truncation looks like -- so the permission to recover has to already be on disk at the
   moment there is something to recover.
2. **The record before the count**, so the file never claims more than it holds. Killed between
   the two, it holds one whole record nobody counted, and that is recoverable. **The other
   order loses the same edit and fails the length check**, blaming the file for the writer.

So the only two states a kill can leave are *n whole records, count says fewer* and *n whole
records and a fragment*. `load` recovers the first, drops the fragment, and says which it did.
The append log truncates the fragment when it next opens the file, because an append landing on
top of half a record would put **every subsequent record 16 bytes out of phase** -- a save file
that parses and is nonsense.

### The bit is what keeps the old guard, and the guard is the point

Relaxing "the length must equal the count" is exactly the kind of change that quietly removes a
check everything downstream depends on. It does not, and the reason is one spare bit: **a file
without `FLAG_APPEND` is held to precisely the check batch 29 shipped.** Longer than its count
is corrupt and refused; shorter is a truncation whatever the flags say. The compacted files are
what every A/B in this document is measured on, and their guard is untouched.
`a_file_that_never_appended_is_still_held_to_its_count` is that claim in both directions, the
same 16 bytes read as corruption or as an edit with one byte of `flags` between them.

Taking a spare bit rather than version 3 is what batch 32's comment on `VERSION` said the next
feature should do, and this is the next feature.

### `--edits` is not `--load-edits` and `--save-edits` spelled shorter

Those two are a **measurement** pair: separately runnable, so one binary can render
`--demo-edits --save-edits f` against `--load-edits f`, and a missing file is a typo that must
be an error. `--edits` is a **world**, and a world nobody has played is a file that is not
there, so a missing one starts a new world.

**Then the sentence it prints is the only guard left**, and that is why it prints one. "Loaded
your world" and "started a different one" are the two outcomes of one flag, they look identical
from inside the renderer, and a mistyped path picks the second silently -- `errors.md`'s rule
about a frame that looks exactly like a frame, aimed at a *world* that looks exactly like a
world. Combining `--edits` with either of the other two is **refused** rather than resolved by
precedence: two readable answers and no way for a reader to tell which one the program picked.

### What batch 33 measured

Everything below is at the fixture's own `edits` vantage -- `--demo-edits --cam-height 14
--cam-pitch -20`, 1280x720, temporal pass off -- and the structure is the 393-block hut. The
log's file is **6,312 bytes** against the exit save's 6,368: the same 393 records, and the
difference is the player block a killed session has nothing to put in, plus -- since batch 34 --
the bar block it has nothing to put in either. **That second number was 6,344 when batch 33
measured it**, and the 24 bytes between then and now are this file's own history of what a
kill costs.

#### The file a kill leaves behind is the world

Side A builds the hut live and saves it the old way **on `voxelcraft-pre33.exe`**. Side B builds
the same hut through the append log and then stops without an exit save. Side C hands B's file
back to **pre33**, a binary that has never heard of `--edits`, `--no-exit-save` or
`FLAG_APPEND`.

| the frame | pixels differing from the hut built live | MAE |
|---|---|---|
| the log's file, replayed on **pre33** | **0** of 921,600 | 0.0000 |
| the same file with 93 records left uncounted -- the kill window | **0** | 0.0000 |
| the same, plus 7 bytes of a half-written record behind it | **0** | 0.0000 |
| only the records that had been **counted** -- what the recovery rule is worth | **67,983** | 1.6371 |
| no journal at all -- what a killed first session used to cost | **348,624** | 6.3769 |

Identical sha256 on the first three, `16baef6c...`, which is also the `edits` vantage's hash in
the capture manifest.

**The bottom two rows are the ones that make the top three mean anything**, and the second of
them is the lesson batch 32 wrote down: an A/B whose side B falls back to side A's input scores
0 for free. Here the frame is demonstrably sensitive to the journal (348,624 pixels) *and* to 93
records of it (67,983), so 0 pixels for a recovered file is a claim rather than an absence of
one. The 67,983 row is the sharper of the two because it isolates the halves: it is what a
reader that trusted the count would have thrown away.

And pre33 handed the same doctored file **refuses it** -- `header says 300 edits (4800 bytes),
file carries 6288`, exit 1. The old guard still bites exactly where the bit is absent.

#### The control

**14 vantages, 0 pixels differing, identical file hashes** against `voxelcraft-pre33.exe`, and
`harness implies` unchanged at 21 claims. **No file under `src/render/` was touched** -- not a
shader byte, not a pipeline.

Exactly zero rather than nearly zero, and for the same structural reason batches 30 and 32 could
say it: **no vantage passes `--edits`**, so `EditJournal::log` is `None` at all fourteen and
`record` cannot reach a file. `Config::journal_out` returns `--save-edits`' path untouched when
`--edits` is absent, so the one function every mode already asked is the only thing that changed
shape.

#### Batch 29's round trip, re-run

Unchanged: **0 pixels and identical hashes at thirteen of fourteen**, with `edits` still at
**298,841** for the reason batch 29 gave -- `--extra-b` can add to a vantage's arguments but not
take them away, so side B builds the hut *and* replays it. Every hash matches, which is what
says the two flags this batch did not change still write and read the same file they did.

#### The half no fixture reaches, and the flag that let it reach the rest

**Batch 33 adds no window-only line, which is new for this subsystem.** `src/app.rs` and
`src/headless.rs` are untouched: the whole feature landed behind `journal::open` and
`journal::save_if_asked`, the two functions every mode already called, so the game got
`--edits` without a line of its own and every line of the batch is reachable from a capture.
That is `save_if_asked`'s signature argument paying a second time -- batch 32 gave it the
player so the deltas and the player could not come apart, and the same funnel is why 33 needed
no third call site.

A real kill is `Stop-Process`, and nothing in `tests/` or the harness can end a capture that
way. **`--no-exit-save` is what makes the rest of it measurable**: a run that ends by writing
the whole journal leaves the same file whether it appended as it went or not, so without that
flag the entire crash-safety half is unobservable -- `lessons.md`'s "a fix nothing exercises is
not a fix", pointed at a feature whose whole purpose is what happens when the ordinary path does
not run. It is negative-tested in a pair, because a no-op claim standing alone cannot be told
apart from one that was never wired up: with the flag the file is byte-identical to what the log
left, without it the same call rewrites it and stamps a player.

### The tests, and the negative test of the tests

`tests/edit_journal.rs` grows eight, all through the public seam `journal::open` -- which is the
real route the game takes, so nothing here is a second implementation of the feature. The one
that could not have been written before this batch is
`a_killed_session_keeps_the_blocks_it_placed`: records, then the process simply ending, then the
file read back. Before 33, "the session did not reach the end" and "the file is empty" were the
same sentence.

Negative-tested twice, and the two stubs come apart cleanly:

- With **`AppendLog::append` a no-op** -- the appending removed -- **exactly the four tests that
  read the file the log wrote fail** and 23 pass, including both refusal tests and the
  `--no-exit-save` pair, which are still true of a build with no feature in it.
- With **the recovery removed** and the appending left in place, **three fail** -- and the third
  is `a_file_that_never_appended_is_still_held_to_its_count`, whose *positive* half is the only
  thing in the file asserting that an append log may carry an uncounted record. Its negative
  half still passes. That is the pairing `lessons.md` asks for, caught doing its job.

`a_killed_session_keeps_the_blocks_it_placed` passes under the second stub and fails under the
first, which is what says appending and recovering are two claims and not one.

---

## Batch 32: the player in the header

A save file that remembers the world and forgets the player is a save file you have to walk
back to. Batch 32 puts **where you stand, where you look and what you have in hand** into the
journal, in a 32-byte block between the header and the records.

### The word batch 29 left, spent the way its comment promised

The header's spare word is now `flags`, and **bit 0 says a player block follows**. That is
the whole of the compatibility story, and it is worth reading because it cost no code:

- A **version-1** file wrote that word as a zero it chose itself. Zero is exactly what "no
  player block" looks like to a version-2 reader, so both versions load through one reader
  with no branch on the number. `a_version_1_journal_still_loads` is the claim.
- The version is bumped to **2** anyway, and what the bump buys is the other direction: a
  pre-32 binary handed a version-2 file says *journal version 2, this build writes 1* instead
  of failing on a byte count and blaming the edit records.
- Twenty-four bits of `flags` are still spare, and the next thing that wants room in this file
  should take one of them rather than a version.

The block is 32 bytes so that header-plus-block stays 8-byte aligned and the record array
behind it is still `bytemuck`-readable in place -- the same arithmetic the spare word's own
comment used to make on its own.

| field | why |
|---|---|
| `pos: [f32; 3]` | feet, in **internal** world Y -- the frame `Record::y` uses, so the file has one coordinate system and cannot disagree with itself about where the ground is |
| `yaw`, `pitch` | radians, because this is what the player *is* and not what a command line asked for; the degrees conversion belongs at `--cam-yaw`, where it already lives |
| `hotbar: u32`, `fly: u32` | `bool` is not `Pod`, and a file format should not depend on what Rust thinks a byte of `true` looks like |
| one spare word | the alignment above, and the same argument as the header's |

**Velocity is deliberately not in it.** A save says where you are, not how fast you were
falling when you closed the window, and restoring a downward velocity into a world whose
chunks have not streamed in yet drops the player through the floor.

### Two refusals, and both are the seed guard's argument again

`PlayerState::check` runs at the file's edge, which is the last place that still knows the
value came out of a file:

- A **hotbar slot this build has no room for** panics in `selected_block`, three call sites
  away and only when the player right-clicks.
- A **non-finite position or angle** renders a black frame, and a black frame is a frame --
  `errors.md`'s failure mode that looks exactly like success.

`--resume` with nothing to resume from is refused for the same reason: no `--load-edits`, or a
journal with no player block, would otherwise render the default vantage and say nothing.

### Who owns the camera: the boundary is the mode's, not the field's

**The window restores without being asked; a headless mode needs `--resume`.**

A window has nothing else that claims a camera, so a loaded journal is the whole answer to
where the player is, and asking for that with a flag would be asking a save file to be told it
is a save file. A capture has `--cam-height`, `--cam-yaw`, `--cam-pitch` and `--cam-submerge`,
all measured from `spawn_position`, and **all fourteen vantages of the measurement fixture are
those flags** -- a journal that silently moved the camera would turn every named vantage into
"wherever the file says", which is not a vantage at all.

The boundary is drawn once, at the mode, and not per field. Under `--resume` the journal
supplies the position, the look angles *and* the hotbar together; without it, none of them.
Splitting it -- letting the file pick the hotbar while the camera flags keep the camera --
reads perfectly reasonable and would mean a `--hud` capture quietly changed the moment a
journal was loaded.

`--resume` also switches off `find_cave` and `find_water`, which are the only two things that
move the camera after the settle. A search that moved a resumed eye would be a capture of
somewhere else that still printed the resumed position.

### The literal `4`

`--hud` drew its hotbar highlight from a literal `4` passed to `draw_hud` from the capture
loop: a picture claiming a selection no player held. It now draws `player.hotbar`, and
`--hotbar` ships at **4** so that every `--hud` capture ever taken still reads the same. That
is what makes this batch's control exactly zero rather than nearly zero, and it is the reason
the flag has the default it has -- `lessons.md`'s "a default is a claim", pointed at a
constant that used to be an argument.

### What batch 32 measured

#### The camera, end to end, at pixel resolution

The claim is that a capture, saved and resumed, is the **same capture**. Not similar: the same
file.

```bash
voxelcraft --screenshot c.png --no-taa --cam-height 137 --cam-yaw 211 --cam-pitch -37 --save-edits c.journal
voxelcraft --screenshot d.png --no-taa --resume --load-edits c.journal
```

| pair | pixels differing | MAE |
|---|---|---|
| the resumed capture against the one that wrote the journal | **0** of 921,600 | 0.0000 |
| the same resumed capture against the **default** vantage | 921,600 of 921,600 | 42.18 |

Identical sha256, `dca96e08...` for both, and 120 chunks drawn on each side.

**The second row is the one that makes the first mean anything, and the first attempt did not
have it.** Run with the default camera on both sides -- `--cam-height 40 --cam-yaw 40
--cam-pitch -14`, which is what side B falls back to anyway -- the pair also comes back at 0
pixels, and it would have come back at 0 with `--resume` doing nothing at all. That is
`errors.md`'s vacuous vantage, reproduced by hand at the first opportunity. A camera the
defaults cannot reach is the whole of the fix.

`--resume` also beats a *conflicting* `--cam-height 400 --cam-yaw 5` on the same journal, 0
pixels against `c.png`, which is the precedence rule as a measurement rather than a sentence.

#### The hotbar, in the only place a picture can show it

| pair | pixels differing | max delta |
|---|---|---|
| `--hud` against `--hud --hotbar 7` | **3,044** | 76 |
| `--hud --hotbar 7` against the same slot **through the journal** | **0** | 0 |

Every one of the 3,044 is inside the 218x58 hotbar strip: scored over the rest of the frame,
`--rect 0,0,1,0.9`, the two images are identical. A change that reached anything else would
have shown up as a second region, which is the assertion worth making about a batch that
touches the overlay.

#### The control

**14 vantages, 0 pixels differing, identical file hashes** against `voxelcraft-pre32.exe`, and
`harness implies` unchanged at 21 claims. **No file under `src/render/` was touched** -- not a
shader byte, not a pipeline -- which is the honest form of "it is free".

The control is exactly zero rather than nearly zero for two structural reasons and neither is
luck: no vantage passes `--load-edits` or `--resume`, so `start_player` cannot reach a journal;
and no vantage passes `--hud`, so the hotbar cannot reach a pixel even if it did. The `4`
above is what makes the second of those true of the `--hud` captures nobody automated.

#### Batch 29's round trip, against a journal that now carries a player

This is the measurement that says the non-read rule holds. Side A now writes a player block
into the journal and side B loads it, so if a headless mode read a saved camera the way the
window does, **every one of the fourteen would move**.

Unchanged: **0 pixels and identical hashes at thirteen of fourteen**, with `edits` still at
298,841 for the reason batch 29 gave -- `--extra-b` can add to a vantage's arguments but not
take them away, so side B builds the hut *and* replays it.

### The tests, and the negative test of the tests

`tests/edit_journal.rs` grows six: the file round trip, the no-player case, the version-1 case,
the two refusals, the offset check, and the trip out through `Player::state` and back through
`Player::restore` -- which is the one that pins the `usize`/`u32` and `bool`/`u32` conversions
the file round trip cannot see.

Negative-tested twice, and the two stubs are worth keeping apart:

- With `save` reading `None` for the player -- the feature removed -- **exactly the three tests
  that name the claim fail** and sixteen pass, including the no-player case and the offset
  check, both of which are still true of a build with no feature in it.
- With the *flag bit* stubbed to zero while the block is still written -- a writer half
  removed -- **four fail**, the fourth being the offset check, which is exactly the test that
  exists for a file whose records start where the header does not say.

What this batch did **not** take is at the top of this file, under "what is persisted, and what
is not", so there is one copy of it.

---

## Batch 30: the deltas at a distance

Batch 29 skipped the coarse levels on the grounds that **"there is no honest answer to which of
a LOD-2 voxel's 512 blocks an edit sets"**. The answer it went looking for does not exist, and
the question was the wrong one. A coarse voxel does not *take* an edit; it **stands for** the
blocks under it, which is exactly what `coarse_trees` does for a tree too big to draw at stride
8 and what `coarse_meadow` does for a blade of grass. What had to be aggregated was never the
terrain -- that agrees already -- it was the journal.

### Three rules, each one `generate_coarse`'s own rule read back

1. **The topmost occupied block wins.** `generate_coarse` says this about terrain in as many
   words: a voxel straddling a shoreline reads as water. So a solid edit takes the voxel when it
   sits at or above the terrain's own top block inside that footprint, and is ignored below it,
   where an opaque face already hides it. Without the second half, one block placed in a cave
   turns an eight-block cube of hillside into planks, visible from across the world.
2. **A voxel clears only when every block it stands for is gone.** One block broken out of 512
   is not a hole at stride 8, and pretending otherwise punches a window through a hillside seen
   from a kilometre away.
3. **"Every block it stands for" is counted against the coarse heightfield, not against LOD 0.**
   The coarse levels have no caves in them, so a count taken from the fine world counts blocks
   this representation never had. **The clear test has to be consistent with the thing it is
   clearing**, which is the one rule here that is not obvious and the one a reviewer should
   check first.

Ties inside a footprint break on a total order -- topmost, then lowest z, then lowest x. The map
hands its entries back in no particular order, and two runs of one world have to paint the same
voxel; it is the argument `save`'s sort makes, one level down.

### An edit does not reach a stand-in that is already resident

`apply` reads the journal **once, at build time**, so a coarse chunk interned before an edit has
no route to it -- and that is precisely the chunk the player is about to walk away into. The
journal is the only place that sees every edit, so it names them: `record` adds the LOD-0 chunk
to a dirty set, `ChunkManager::absorb_dirty` walks the ancestors, and there are two schedules
for the same work and one statement of what is stale.

- **`update`** rebuilds a few per frame against the streaming budget, **before** missing chunks:
  an absent chunk shows nothing, a stale one shows something wrong. `insert_local` swaps the
  rebuilt chunk in when it lands, so nothing is removed and no frame has a hole in it.
- **`rebuild_dirty_now`** does the lot on the calling thread, for a capture that has no next
  frame to be corrected in. **It is not a convenience** -- but the reason given here was wrong
  until batch 31 and is worth reading as a correction. This said the obvious alternative, a
  second settle after the edits, moved `open-sea` by 52 pixels and `shore` by 56 in a run with
  no edits in it, *because `update` is not idempotent on a settled world*. It is idempotent:
  four extra calls at an unmoved camera are 0 pixels at all fourteen vantages. What moved those
  two captures was the settle running at the camera the capture renders from, which the path
  had never planned at -- a correction, not noise, and batch 31 now takes it on purpose before
  the edits land. The surviving reason to build here is the plain one: a settle spends its
  rebuild budget a few chunks per frame, and this call site has no next frame to spend it over.

---

## What batch 30 measured

### The fix, at the default LOD settings

**A `--streaming-factor` low enough to make the near ground coarse is a trap**: the camera ends
up *inside* a stride-8 voxel, because a coarse column's height is sampled at its centre and the
eye is 14 blocks above the fine ground, not above that. The frame is then a close-up of a brown
wall and 643,858 pixels move for reasons that have nothing to do with edits. The honest way to
put a structure at a coarse level is to **look at it from far enough away that the shipped LOD
rules make it coarse**, which is also what the defect was always about.

So: build the hut through the normal edit path, save its journal, and replay it under a camera
350 blocks up looking straight down -- far enough that the ground draws at LOD 1.

```bash
voxelcraft --demo-edits --cam-height 14 --cam-pitch -20 --save-edits hut.journal --screenshot a.png
voxelcraft --load-edits hut.journal --cam-height 350 --cam-pitch -88 --screenshot b.png
```

| camera | pixels differing vs `voxelcraft-pre30.exe` | MAE | max delta | what draws there |
|---|---|---|---|---|
| `--cam-height 300` | **0** | 0.0000 | 0 | still LOD 0 -- the control row |
| `--cam-height 350` | **284** | 0.0111 | 78 | LOD 1 |
| `--cam-height 500` | 180 | 0.0070 | 76 | LOD 1, smaller |
| `--cam-height 800` | 110 | 0.0035 | 76 | LOD 2 |

**The 300-block row is the one that makes the others mean something.** It is the same command
with the same journal at a height where LOD 0 still covers the hut, and it moves nothing --
so the rows below it are the coarse path and not some other thing the journal does.

**Every differing pixel at 350 is inside one 25x25 box** at (628, 343)-(653, 368), which is the
hut and nothing else: 11 blocks at 350 blocks' range is 20 pixels, and the stride-2 rounding
takes it to 25. A change that reached anything else in the frame would have shown up as a second
region, and that is the assertion worth making about a batch that edits a chunk builder.

### The control

**14 vantages, 0 pixels differing, identical file hashes** against `voxelcraft-pre30.exe`, and
`harness implies` unchanged at 21 claims. **No file under `src/render/` was touched** -- not a
shader byte, not a pipeline -- which is the honest form of "it is free", the one `errors.md`
will accept where it rejects `shaderstats`' binary size.

The reason the control is exactly zero rather than nearly zero is structural: `apply_coarse`
returns on an empty map before it looks at anything, and **no vantage in the fixture carries an
edit at a coarse level**. `a_coarse_chunk_with_no_edits_under_it_is_untouched` is the same claim
as a test, and it is deliberately kept beside four positive ones -- a no-op claim standing alone
cannot be told apart from a control that was never wired up.

### Batch 29's round trip, re-run

Unchanged: **0 pixels and identical hashes at thirteen of fourteen**, with `edits` still at
298,841 for the reason batch 29 gave -- `--extra-b` can add to a vantage's arguments but not
take them away, so side B builds the hut *and* replays it. Every hash matches the pre-30 run
byte for byte, which is what says `rebuild_dirty_now` corrects the stand-ins without disturbing
anything else.

**It did not survive the first attempt, and that is the entry worth reading.** Reaching the
rebuild through a second `settle()` put `open-sea` at 52 pixels and `shore` at 56 against a
round trip that had been clean for batch 29. Three experiments, in this order, are what made it
cheap: the `--load-edits` side was bit-identical to pre-30, so the aggregate was not the cause;
stubbing the rebuild out while keeping the settle reproduced the 52 pixels exactly, so the
rebuild was not the cause either; and a second settle with **no edits anywhere in the run** gave
the same hash again, which named the real mechanism and took it out of this batch.

**Batch 31 then found the elimination had one step left in it.** Every experiment above varied
the *edit* path and none varied the *camera*, so all three eliminations were sound and the
conclusion drawn from them -- "so it is `update` itself" -- was not: a second `settle()` also
plans at whatever camera it is handed, and at that point in the capture path the camera is the
one `find_water` just chose. The extra call was correcting a stale plan. The elimination that
was missing is the cheap one: the same extra call at an *unmoved* camera, which is 0 pixels at
all fourteen. **A sweep that varies one input and eliminates three causes has eliminated three
causes, not named the fourth.**

### What it costs

Nothing on the frame path, and nothing on any measurement in the project. Per frame the main
thread pays one `take_dirty` -- a lock and a drain on an empty set. Per chunk *built*, with a
journal loaded, it pays a scan of the journal's chunks; the number and the bound it implies are
in [`errors.md`](errors.md).

### The tests, and the negative test of the tests

`tests/edit_journal.rs` carries the three rules one test each, plus order-independence and the
free-by-construction claim. `tests/lod_stream.rs` carries the half no single chunk build can
show -- that a settled world rebuilds its stale stand-ins after an edit **and then goes quiet
again**, the companion to `stationary_camera_streams_nothing` directly above it. The second
assertion is the load-bearing one: the dirty set is drained rather than read, and a version that
re-requested every frame would pass the first half while starving the budget a moving camera
needs, which is the defect `errors.md` already records against prefetching the ancestor chain.

Negative-tested the way batch 29's were, twice. With `apply_coarse` stubbed to return 0, exactly
the four tests that name the aggregate fail and nine pass. With the dirty absorb stubbed to an
empty list, exactly the one test that names the rebuild fails and every other test in both files
passes.

---

## What batch 29 measured

The claim the batch stands on is that **`worldgen + deltas` is the chunk you edited** -- because
that is what makes an unload invisible, not merely what makes a save file work.

### The round trip, at fourteen cameras

`bitexact` renders side A then side B for each vantage, so one journal path serves the whole
sweep: A builds the structure through the edit path and writes the file, B replays it.

```bash
harness bitexact --extra "--demo-edits --save-edits target/harness/edits.journal" --extra-b "--load-edits target/harness/edits.journal"
```

**0 pixels differing and identical file hashes at thirteen of the fourteen.** The fourteenth is
not a failure of the journal; it is the section below.

**Nine of those thirteen are vacuous, and they say so themselves.** `--demo-edits` builds in
front of the camera, and at nine vantages the structure is out of frame -- side A's hash comes
back byte-identical to the batch-28b capture of the same vantage, so a journal that replayed
nothing would have passed there too. `lessons.md`'s "a verification vantage that shows nothing
still counts itself" is exactly this, and the way to see it is to look for two rows that match.

The four where the edits move the frame are the ones carrying the claim, and each is a pair:

| vantage | journal against **no** journal | journal against the same edits applied **live** |
|---|---|---|
| `shore` | 333,366 px, MAE 1.52 | **0** |
| `terraces` | 584,812 px, MAE 4.97 | **0** |
| `underwater` | 59,976 px, MAE 0.68 | **0** |
| `cave` | 921,599 of 921,600 px, MAE 30.97 | **0** |
| the `edits` camera, by hand | -- | **0**, MAE 0.0000 |

The left column is not decoration. [`errors.md`](errors.md) records that a no-op claim standing
alone cannot be told apart from a control that was never wired up, and every entry in the right
column is a zero. `cave` is the largest because it is the other branch of
`build_demo_structure`: four glowstones in an unlit chamber, which moves all but one pixel of
the frame.

### The fourteenth vantage, and why it answers a different question

`edits` is the one vantage whose **own arguments** carry `--demo-edits`, and `--extra-b` can add
to a vantage's arguments but not take them away. So side B replayed the journal **and** built
the structure a second time. The second build landed on top of the first, because
`build_demo_structure` drops until it finds solid ground and a replayed plank floor is solid:
the single structure spans internal Y **136-141** and the doubled one reaches **142**, at **586**
recorded edits against 393. **298,841 pixels**, which is the fixture answering "what if you
build the hut twice".

It is a positive result read sideways -- the second builder found ground five blocks up
*because* the replay had already put a floor there. Run by hand at the same camera and the same
recipe with the structure built once, the pair is 0 pixels at MAE 0.0000, which is the last row
of the table above.

### The control, and the shape of the cost

A build with neither flag is **bit-identical to batch 28b at all fourteen vantages**
(`bitexact --exe-b voxelcraft-pre29.exe`), and `harness implies` is unchanged.

**No file under `src/render/` was touched** -- not a shader byte, not a pipeline. That is the
honest form of "it is free", and it is the form to use: `errors.md` rejects `shaderstats`'
quantised binary size as proof that two builds are the same program. What the frame pays is one
hash lookup against an empty map per chunk built, off the render path entirely.

### The tests, and the negative test of the tests

`tests/edit_journal.rs` checks the same claim block for block and light for light in a tenth of
a second, and says which voxel disagreed -- which is why it exists rather than leaving a
fourteen-vantage sweep to find out. It carries the seam the two lighting routes meet at:
`stream::relight` derives its column heights from `gen.height(x, z)` while `WorldGen::generate`
fills them from `column_field` at stride 1, and **nothing else in the tree asks those two to
agree**. The `assert_ne!` against the unedited chunk sits in the same test for the reason the
left column above exists.

The fixture was negative-tested the way batch 28's guards were: with `journal.apply` stubbed
out, exactly the two tests that name the claim fail and the other six pass.



