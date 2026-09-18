
# Lessons: read this when you are planning a batch

Keyed on **the kind of batch you are about to plan**, because the previous version was 451 lines
of prose read in full to answer a question asked once, and prose is not indexed by what you are
doing.

The neighbours: [`pitfalls.md`](pitfalls.md) fires when you type a command.
[`errors.md`](errors.md) is what has already been tried and lost -- **read the section for your
subsystem there before you design anything**, because roughly half of what used to be in this
file was a specific rejection and now lives there with its number.

---

## Any batch

- **Build the revert binary before you write a line.** Every A/B claim needs a build *without*
  the feature in it, and the cheapest moment to have one is before the feature exists. A flag
  that switches the feature off is not a substitute.
- **The control's constant must predate the batch**, or the control is measuring itself. Take the
  hash on the untouched tree, and also assert that the *enabled* path differs from it -- otherwise
  a feature that silently did nothing passes.
- **Name the vantage or do not quote the number.** A per-feature millisecond is meaningless when
  the cost depends on what is on screen: water is bounded below at exactly zero and above by how
  much sea is in the frame.
- **Cost estimates miss in both directions and by large factors.** Smooth lighting came in 4x over
  its estimate, fog at a fifth of one, the cross-fade at +4.5% against a guess of +100%. Only the
  bench settles one.
- **A batch's verification story can change shape mid-batch, and that is fine.** Batch 8 expected
  every capture to move and only *submerged* terrain changed, which made the method stronger.
  Batch 11 planned a test it could not have, because nothing in `tests/` touches wgpu.
- **One batch per session, and it outranks curiosity.** If the numbers look wrong but the code
  checks out, do not investigate -- finish, write an entry at the front of the queue saying what
  the number was, what was expected and what is ruled out, and stop. A mid-session investigation
  is unbounded and usually ends with two half-finished things.

## Planning a *look* change

- **Measure the look, not only the error.** A metric can bottom out cleanly, with a textbook
  minimum and a derivation 2% off it, and still be scoring the wrong thing -- MAE only partly
  charges for speckle, which was the actual complaint.
- **After the second metric disagrees with your eyes, stop building metrics and look at a
  picture.** Three in a row were confounded once, each by a different mechanism.
- **A ground truth can come from an existing flag even when no flag can be a control.**
  `--streaming-factor 8` turned "does this look better" into 15.03 code values against 1.02.
- **Price a compensation through the medium it will be seen through**, not against the thing it
  replaces. A per-channel fit to a carpet's colour measured *worse* than one scalar, because the
  haze lifts blue faster than red -- the space a constant is authored in includes the atmosphere
  in front of it.
- **A multiplier on linear albedo reads at its 1/2.2 power.** Anything authored by eye and applied
  in linear needs raising to about 2.2 first, **in the constant**, because a `pow` in `resolve` is
  the thing batch 2 exists to keep out.
- **A still-frame A/B cannot see a term that should depend on a surface and depends on the camera
  instead.** When a batch adds something a *secondary* ray reads, point `--cam-dolly` at the
  secondary ray. And since batch 23, point `--anim-rate` at anything that moves on its own.
- **When a batch adds a grid, write down where a cell's value was sampled and where the shader
  thinks it lives, before the first capture.** Batch 35 filled each texel from the *centre* of
  the cell and read it back as though index `i` sat at the cell's *edge* -- a uniform two-block
  displacement of every shadow, smaller than the ramp it was blurred over, and therefore
  invisible in the A/B, in the control and in every metric the fixture has. It was found by
  rereading the code. **A half-texel is the shape of error a look batch cannot measure its way
  out of.**

- **Rank a look against this file's own calibration ladder, never against a pixel count.**
  Batch 63 moved **759,140 pixels of 921,600** at `default` and was invisible to the user in an
  image slider; the same frame is **MAE 1.2185**. The count answers *did the feature fire*, and
  only the MAE answers *can anyone see it*. Every reference point this project has, all at
  `default` and all on file:

  | MAE | verdict | where |
  |---|---|---|
  | **0.372** | invisible | batch 60's transport half alone |
  | **1.2185** | invisible in a slider | batch 63 at its physical strength |
  | **2.2248** | shipped | `--probe-sun` at `PROBE_SUN_GAIN` 0.10 |
  | **2.87** | rejected by eye | [`ADR 0001`](adr/0001-no-elevation-form-factor.md) |
  | **3.0379** | shipped | batch 63 at `SKY_TINT_SAT` 2.5 |
  | **5.7074** | too loud, rejected by the user | `--probe-sun-high` at 0.25 |
  | **5.98** | shipped | batch 65's specular, the loudest rung in the sequence |

  **The ladder is not monotone in what ships, and that is the information in it**: 5.98 shipped
  and 5.7074 did not. The number bounds *visibility* and never decides taste -- quote an MAE
  beside every pixel count, and take the verdict from the user.
## Planning a *performance* change

**Grouped after batch 47, when this section reached twenty-six entries and stopped being
readable in the way the file's own premise demands.** Nothing was added or removed in the
regrouping; the bullets are the ones the batches wrote.

### What the pass is bound by

**Six results, and five of them rule something out.** The arithmetic says what cannot be the problem; only a bench has ever said what is.

- **Count the lanes before you open the occupancy table.** Occupancy is a ceiling and has twice
  failed to predict a time here; warp fill is a throughput defect and showed up immediately. When
  both are wrong at once, only a sweep that separates them says which paid.
- **"It costs X while executing nothing" has two mechanisms wanting opposite fixes** -- occupancy
  wants fewer registers, code size wants the code *gone*. Only a register count tells you which.
- **"It costs X while executing nothing" has a third sibling: costing X while compiling to the
  identical binary.** This one is neither occupancy nor code size -- `shaderstats` reads
  **28,151 instructions, 615,424 bytes and 96 registers on both sides** of batch 41, because
  the change is one immediate operand and the saving is loop iterations that do not run. The
  register count cannot tell you about this one; only the bench can.
- **And a fourth: removing the code and getting slower.** Batch 42 took 271,872 bytes out of
  `resolve` -- 85% of everything glass had added to it -- by giving the pane's transmitted leg
  the primary dispatch instead of a second inlined copy of `shade_water`. The frame was
  **+0.09 ms slower** at `coastline`, bit-exact at all fifteen vantages. **Code size is a
  hypothesis about time, not a measurement of it**, and this is the first time here that the
  two have pointed in opposite directions.
- **To ask whether something is memory-bound, build the variant that changes only its memory
  behaviour.** Batch 47 wanted to know whether a per-face cache was worth building, and got the
  answer without building one: point all nine of the gather's reads at the *same cell*. The
  instructions are identical -- `resolve` moved 640 bytes -- and the memory system sees one
  address instead of nine, which is **better** than any cache, since it pays no tag compare and
  no barrier. It recovered **10-13%** of the block, so the block is instruction-bound and the
  cache was dead. **A diagnostic that isolates one factor by holding the other fixed beats both
  a profiler and a prototype**, and it cost twenty seconds.
- **The vantage where a feature does the *least* work is the one that identifies the mechanism.**
  Batch 45's failed build cost the most at `default` -- a frame with water only at its right
  edge, running the new arm almost nowhere. A regression that is largest where the feature is
  least used cannot be the work; it is the code. That reading costs one extra row in a bench
  that is already running and it is the cheapest separator this project has for batch 12's two
  mechanisms.

### Where a cost actually lives

**The pass that pays is routinely not the pass that runs the code**, and the function you are looking at routinely has a caller you are not.

- **A feature's cost and its batch's cost are different quantities, and the pass that pays may not
  be the pass that runs it.** All of batch 14's work was in `march`; all of its cost was in
  `resolve`, which never draws a blade.
- **Check who else calls the function before claiming a cost bound.** `shade_water` calls
  `sky_color` unconditionally, which has surprised three batches in a row.
- **Cost is set by the occluder's kind, not the sample budget.** Four analytic samples cost +0.82
  ms where four world marches cost +6.3. **Price one sample before tuning a budget.**
- **A secondary ray's cost is not its traversal, and asking `how far does it go` is the wrong
  question.** What a ray costs is the walk *plus what happens where it lands* -- and in this
  engine a secondary hit calls `shade_hit`, which casts a ray of its own. Refraction's shadow
  ray is **4.1 ms** of a coastal `resolve` against 1.5 for the whole reflection reach.
- **Merging two dispatches into one costs the common path what it saves the rare one.** The
  reason is visible in the source: after the merge the primary `shade_hit` call takes variables
  a branch may have overwritten, where before it took values the compiler could see through.
  **A shared call site is not free; it is paid for in constant propagation**, and the pass that
  pays is the one that runs on every pixel.

### `resolve`'s cost is its register count, and which feature owns them does not matter (batches 66 and 67)

**Two entirely unrelated features cost the same number for the same 16 registers.** The test is
`cave`, where glass draws nothing and batch 65's specular changes zero pixels, so both are pure
dead weight -- and all four counts are selectable from one binary, because both are specialization
bits:

| build | registers | `resolve` at `cave`, vs the 80-register build | 1 se |
|---|---|---|---|
| neither | **80** | -- | -- |
| glass only | **96** | **+0.156 ms** | 0.005 |
| specular only | **96** | **+0.151 ms** | 0.018 |
| both | **128** | **+0.441 ms** | 0.014 |

A transmissive material with a traced reflection and a Fresnel sky lobe are indistinguishable when
neither draws anything. That is what roadmap P8 had been asking since batch 42.

- **Ask how many times a function is inlined before asking what a line in it costs.** `shade_hit`
  is inlined four times -- the primary surface, water's two legs, glass's transmitted leg -- so a
  term in its body exists in four copies and the allocator budgets for all of them. Passing a
  literal `false` at the three secondary sites took `resolve` from 128 back to 96 and recovered
  **83-87%** of batch 65's bill.
- **Ask where a value dies before asking what an expression costs.** Batch 67 stubbed each of the
  specular's three operations in turn and every stub read 80, so it was none of them; folding the
  weight to one scalar changed nothing, because the compiler already schedules that. Written
  beside the return, the term pinned `n` and `rd` live across the nine-cell gather, the probe tap
  and the shadow march. Hoisted to just after `n` is finalised, one `vec3` crosses instead.
  **96 to 80, for moving a line rather than changing what it computes.**
- **A negative stub means "not the sole cause", never "not a cause".** Batch 66 stubbed glass's
  two secondary arms separately, read 96 each time, and struck both. Stubbed **together** they
  read **80** -- allocation is a `max()` and not a sum. The strike was wrong, the next session
  un-struck it, and that is how P8 closed.
- **So two ceilings have to fall in one batch, and that is the general form.** The shipping count
  is a `max()`, so two independent features each holding 96 means fixing either alone pays
  **nothing**. Find every arm sitting at the peak before costing a fix.
- **NVIDIA allocates in blocks: a register fix is worth a whole step or worth nothing.** Hoisting
  glass's own reflection above its dispatch shed about five registers and read 96 unchanged. **And
  do not price a budget by interpolation** -- 32 registers cost **1.41x** two lots of 16, so there
  is a step between 96 and 128.
- **Code size is not it, and that is now proven three times.** Batch 42 removed 271,872 bytes of
  glass and the frame got *slower*; batch 65's term made the module smaller while costing 0.3 to
  0.5 ms; batch 66 removed 200,000 bytes and the register count did not move at all.
- **`f16` halves what a live value costs, and this driver does pack it (batch 69).** Measured at
  the same live range the hoist above is about: 32 scalars crossing the gather read **96 registers
  as `f16` against 128 as `f32`**, two whole steps. **What packs is storage across a live range and
  not arithmetic** -- an arm that chained `sin()` read 128 on *both* sides, because a transcendental
  widens to `f32` and every value feeding one is `f32` in a register however it was declared. So
  `f16` answers the same question the hoist does -- *what is this value while it waits* -- with a
  type instead of a position. Roadmap **P14**; what is unmeasured is whether this shader has a
  step's worth of `f16`-safe live range in it, and `spec_pre` alone is 3 scalars.
- **A pressure test measures nothing unless recomputation is expensive (batch 69).** The first
  attempt at the sweep above was a pure multiply-add chain and read **80 at every width up to 48
  scalars** -- the allocator rematerialised it past the gather rather than holding it, so no
  pressure was ever created. One independent `sin()` per vector fixed it. **An inert pressure test
  reads exactly like a clean negative**: the run exits 0, the table is internally consistent, and
  nothing in the output says the experiment did not happen. Same family as the `harness`
  stale-renderer trap in [`pitfalls.md`](pitfalls.md).
- **Check that the type survived lowering, not just that it compiled (batch 69).** A WGSL `f16`
  that naga promotes to `f32` compiles, runs, and reports a register count -- and that count is
  about a shader with no `f16` in it. `shaderstats --out DIR` writes the SPIR-V; scan it for
  `OpTypeFloat` width 16 and `OpCapability Float16`. Two words of Python against a whole wrong
  conclusion.

### Truncations and ramps, which is the lever this project reaches for most

**Four of the five shipped performance wins here are a distance made shorter**, and every one of the failures below is too.

- **A ramp saves nothing unless it reaches zero.** Tune it to where the work stops being *worth
  doing*, then check that the vantages you do not care about actually reach zero.
- **Price the shape of a distance curve, not just its endpoints.** The shadow ray's first
  eight blocks cost three to five times per block what its last sixteen do, because most rays
  terminate early -- so the near blocks are paid by every ray and the far ones only by
  survivors. **A truncation is cheap where the look is, and the curve is what says so.**
- **Then measure what the removed *band* contains, because that is the look cost and the
  endpoint is not.** Batch 40 priced the whole shadow ray at 4.1 ms; batch 41 asked what each
  segment of it was actually doing and found the answer wildly uneven -- deleting the march
  past 16 blocks moves **1,309 pixels at max delta 1**, and deleting the first 16 as well moves
  **369,628 pixels at `shore`**. Same feature, same units, a 280x difference in what the cut
  costs. **Rank a truncation by the content of the band it removes, never by the size of the
  thing it truncates.**
- **A constant another batch backstops is cheaper to cut than its own documentation says.**
  `WATER_SHADOW_DIST`'s comment had described what truncating it gives up since batch 8, and it
  had been wrong since batch 37: `shade_hit` hands the interval past the march to the light
  envelope, so a shorter budget moves a handoff instead of deleting a shadow. **When a batch
  adds a second occluder, a second sampler or a second reader, go and re-read what the first
  one's constants claim they cost** -- nothing in the code changed, and the price of every one
  of them did.
- **A truncation that worked on one ray does not transfer to another, and the medium is why.**
  Batch 41 cut water's shadow march 48 -> 16 for 1.68 ms because water is *transparent* to a
  shadow ray, so the ray grinds for hundreds of blocks. Batch 42 tried the same on the air
  budget -- 220 -> 64 -- and got **-0.040 ms of a 1.168 ms march**: on land the ray either hits
  terrain at once or escapes to open sky at once, so 97% of its cost is in the first few dozen
  blocks. **Ask what stops the ray before assuming where its cost is.**
- **Batch 44 is the third reading of that and the sharpest, because the two rays are in the same
  function.** `shade_water`'s refracted leg hands its hit a 16-block shadow budget and its
  reflected leg hands its hit 220, and the asymmetry looked like batch 41's millisecond lying on
  the floor. Cutting the reflected one to 16 is **-0.044 +/- 0.029** and *deleting its shadow
  outright* is **-0.019 +/- 0.056**. The refracted hit is under the sea and the reflected hit is
  a hillside in air, so one grinds and the other stops -- **the lever is the medium at the hit,
  not the leg, not the budget, and not which function it lives in.**
- **A truncation's saving and its look cost are measured on different axes, and a cheap one can
  be the expensive one.** Batch 41 bought 1.672 ms for 1,309 pixels at max delta 1. Batch 44's
  reflection reach buys **0.446 ms for 5,442 pixels at delta 86** at the same vantage -- four
  times less time for four hundred times the picture -- because the band it truncates holds a
  headland where batch 41's held nothing. **Rank a truncation by what is in the band, and get
  the band's contents from a picture before the cost curve tempts you.**

### Folds, gates and specialization

**`SPEC_MASK` is the sharpest tool here and the one whose results transfer least between sites.**

- **A folding trick that worked once does not transfer.** The fold is a property of the pass, not
  of the source shape; check every pass the constant reaches, in both directions.
- **Making a control free is not making the build faster, and a batch should say which it did.**
- **An override that gates code inside a hot inlined function must be the *whole* gate.** The
  house style for a switch is `SPEC_X && (frame.flags & FLAG_X) != 0u`, and it is free until
  the arm it guards sits in a function the pass inlines several times -- then `frame.flags` is
  a run-time value, the driver keeps *both* arms in *every* copy, and the feature costs what it
  was meant to save. Batch 45 measured the same lighting change at **+1.000 ms** and
  **-1.584 ms** at `terraces` with nothing between the two builds but that expression. Let the
  pipeline key be the only copy of the decision, which is what the two numeric overrides
  already do.
- **A win's mechanism tells you its scope, and the scope is usually narrower than the rule.**
  Batch 45 folded one gate for 2.58 ms and the obvious generalisation was the other eight.
  Batch 46 measured them: **-0.119 ms at one vantage and nothing at four**. The cost was never
  the gate shape -- it is the shape *times the arm behind it times how many copies the pass
  inlines*, and only the first of those three is what a rule written from one result captures.
  **Before generalising a win, write down which of its factors the next case actually shares.**

### Running the measurement

**Cheap first, and one variable at a time.** Every entry here was learnt by spending a session on the expensive version.

- **Sweep before you read, not after.** Batch 40 ran three one-constant diagnostic builds,
  twenty seconds each, and they **inverted the ranking of three research documents in both
  directions**: the fix all three ranked first bought water exactly zero, and the one none of
  them mentioned was 31% of the pass. The mechanisms in those documents were right and every
  checkable claim held -- it is the *ordering* that a document cannot supply. See
  [`research-review.md`](research-review.md)'s postscript.
- **Price the ceiling before designing the recovery.** The same batch first measured what the
  whole block costs, by replacing it with a model that already existed -- batch 45's flat
  lookup. Without that number, 10-13% would have been meaningless; with it, the decision was
  immediate. **Ceiling first, then the share of it your idea can reach.**
- **A free screen that predicts the bench is worth building before the bench.** Batch 46's eight
  candidates were ranked by `shaderstats` byte delta in two minutes with no GPU, and the ratio
  it reported -- 12,544 bytes against batch 45's 60,928 -- was the whole answer. The benches
  confirmed it and told nobody anything new. **When a mechanism has a cheap offline proxy, run
  the proxy across every candidate first and bench only what it ranks highest** -- which is what
  `shaderstats` is for and what four batches used it as an afterthought.
- **Two constants moved in one diagnostic build cannot be told apart afterwards, and a later
  session will quote the pair as one.** Batch 40 benched `WATER_REFLECT_DIST` 768 -> 384 *and*
  its fade 512/1024 -> 256/384 together for -1.476 ms; `water.md` then attributed the whole of
  it, and a pixel count that turned out to be the fade's, to "halving the reach". Separated,
  the reach is -0.746 and the fade -0.553. **One constant per build, including in a sweep whose
  point is only to rank.**

### Counting beats timing whenever the thing is countable (batch 53)

**Fifty-two batches ranked changes in milliseconds, and `march`'s work is an integer.** The
whole of this section is one batch's, and it is here rather than in `gpu.md` because none of it
is about the marcher.

- **Before you design a measurement, ask whether the quantity is an integer.** A count cannot be
  moved by heat soak, by power state, by a game window left open, or by the 1.8x drift this
  project attributes to sittings. `harness march` was thirty lines of reader over a buffer the
  shipping build had been filling for twenty batches, and it made three candidates rankable in
  four minutes with no GPU timing at all. **The instrument that already exists and has no reader
  is the cheapest one you will ever build.**
- **Price a candidate with a build that is deliberately *wrong*.** Patch one literal to a value
  that cannot ship -- a uniform-water leaf stepping 16 when it may only step 4 -- build, count,
  throw the picture away. It bounds what the exact version could ever buy, in twenty seconds,
  before the format question is opened. Batch 53 dropped one of three candidates on its ceiling
  alone and built the other two. This is `price the ceiling before designing the recovery` with
  the correctness requirement dropped, which is what makes it cost minutes instead of a session.
- **A picture of a distribution is not the distribution.** Every iteration heatmap in the
  vantage set shows a thin bright band of rays grazing water, and it is compelling and it is a
  twentieth of the pass: the top 1% of pixels hold 3 to 9.5% of the steps and the median pixel
  at 14 to 24 is the frame. An optimisation aimed at what the heatmap draws attention to would
  have been aimed at almost nothing. **Get percentiles before you get a hypothesis.**
- **A bit-exact result and a count are one finding, not two.** A pure traversal optimisation is
  exactly what `bitexact` cannot distinguish from a feature that never fires -- the reading batch
  51 shipped nothing over -- so neither number means anything alone. Quote them together: *0
  pixels at eighteen vantages, -4.90% of DDA steps.*

### Scaling the feature beats benching it harder, when the cost is small (batch 55)

**A tap per shaded surface measured between -0.055 and +0.017 ms across five vantages, mixed
sign, every figure inside the fixture's own resolution.** Eight more rounds would not have
separated those from zero, and on a laptop ninety minutes into a sweep they would have been
worse: `terraces` read +0.017 +/- 0.002 early and +0.048 +/- **0.048** late, the same quantity
with a standard error twenty-four times larger.

**What settled it was tripling the feature, not the rounds.** `PROBE_TAPS` 1 to 3 is one literal;
`cave` read **-0.040 +/- 0.000 at both**, every round identical. A cost that does not move when
the work triples is not the work -- it is the code existing, which `shaderstats` confirmed at
582,400 / 583,680 / 584,960 bytes and 96 registers throughout. **And that answered a better
question than the one being asked**: not *is one tap cheap* but *does the cost scale with the
coefficient count*, which is what decides whether a directional field is affordable at all.

- **When a delta is near the noise floor, multiply the feature rather than the rounds.** Rounds
  fight the machine; scaling the work changes the signal. It is `price a candidate with a build
  that is deliberately wrong` aimed at the *other* end -- make it deliberately **bigger** instead
  of deliberately incorrect.
- **Batch 45's rule has a mirror.** That batch: *a regression largest where the feature is least
  used cannot be the work; it is the code.* This one: *a cost that does not move when the work
  triples is the same statement.* Both separate code from work without a profiler.
- **A vantage where the feature does almost nothing is worth a row for this alone.** `sky` reads
  -0.020 with almost no shaded surfaces -- half of `cave`'s, in a frame with a fraction of the
  work -- which is the floor showing directly.

### An architecture is a pair of measurements, and one of them is nobody's instinct (batch 56)

**Batch 55 measured what a feature costs and it was the wrong question on its own.** The
interesting number for a *replacement* is never the cost of the new thing -- it is the cost minus
what the new thing retires, and the second half is the one that gets skipped, because a feature
branch is a natural thing to build and a feature branch that *deletes* the code it replaces is
not.

The two together:

| | `default` | `cave` | `sky` |
|---|---|---|---|
| the tap alone costs | -0.055 | -0.040 | -0.020 |
| tap **and** removal saves | **+0.091** | **+0.130** | **+0.030** |

- **Build the deleting version, not only the adding version.** `--probe-ambient` is one override
  that both takes the new path and gates out the terms it retires. It is the same cost as the
  adding build and it answers the question the adding build only half answers.
- **Make the second flag imply the first, at the parser.** The removal without the replacement
  shades from nothing, renders a frame and reports a millisecond -- and **the fastest build is
  always the one that computes nothing**, so that reading looks like a triumph. Putting the
  implication where the `Config` is parsed means no consumer can construct it.
- **The ordering across vantages is the mechanism check, and it is free.** Batch 55's cost did not
  grow with surface density, so it was code; batch 56's saving is largest at `cave` and smallest
  at `sky`, so it is work. **Neither reading needed a profiler and neither is available from a
  single vantage** -- which is the argument for benching the cheap frame alongside the expensive
  one even when it is obviously going to show nothing.
- **A deliberately incorrect build can answer a design question that no correct build could reach
  yet**, and both of these were. The picture is discarded; what survives is a millisecond and a
  byte count.

### A roadmap entry's architecture is a hypothesis, and its *cost* is where it hides (batch 58)

**R2 had been on file for two batches as "the rung that pays for the apron".** The entry said a
bounce needs real voxel geometry and the flood's own levels, both chunk-local, so the field
would stop being a pure function of world position and the whole invalidation graph would
arrive. That is a coherent argument and it is what made R2 look like the expensive rung.

**It was a premise, and one question dissolved it: which term dominates?** Outdoors, one bounce
is sunlight off the ground; the ground is exactly what the height field describes; and an
exposed column top is at sky level 15 by construction, so the flood is not consulted either.
The shipped bake rides batch 57's own horizon march, stays a pure function of position, and
costs **+681 us -- 1.36x, against an entry that said to expect several times.**

- **When an entry names an architecture, find the sentence that makes it necessary and ask what
  fraction of the effect lives on the other side of it.** Here the necessary-making sentence was
  "a bounce needs real geometry"; the fraction on the other side was most of it. **The entry was
  right about what a *complete* bounce needs and wrong about what the first rung needs**, and
  those are different questions that one paragraph was answering at once.
- **Two batches in a row have now sidestepped P5b the same way**, which is worth reading as a
  pattern rather than as luck: a field that does not know which chunk is asking has no seam, and
  reaching for a world-space function instead of a chunk-local one is cheap enough to try first.
- **And the honest other half: the entry was not merely pessimistic, it was wrong in both
  directions.** It also promised R2 would *collect* batch 56's +0.03-0.13 ms, and it collects
  none of it -- that figure is what a **pre-composed** field lets `resolve` delete, and a bounce
  that multiplies `floor_rgb` deletes nothing. **Check a claimed saving against the mechanism
  that produced the number, not against the rung it was filed under.**

### An entry's "cheap" is about the work; ask which resource pays (batch 59)

**The rung table called R4 the cheapest entry in the file and it is the most expensive rung the
main sequence has shipped**: +0.321 ms of `resolve` at `cave`, +0.340 at `default`, +0.150 at
`sky`, against R1's 0.02-0.04 and R2's nothing. The estimate was not wrong about the *work* --
three lamps and a widened nibble really is a small batch. It was wrong about *which resource
pays*, which is the question `CLAUDE.md`'s scarcity table exists to make you ask first and which
the word "cheap" quietly answers for you.

- **A rung that replaces a constant with a *field* reads one tap and can be free or better.** R1
  and R2 both did, because their output is a function of position and a bake can hold it.
- **A rung that replaces a constant with *content* widens a per-cell store, and every reader
  pays.** R4's output is a function of position too and still could not go in the bake: what
  varies is the value at every *cell*, and the flood was already the cheapest store of that.
  **"Can it be a field?" is a different question from "is it a function of position?"**, and only
  the first one predicts the cost.
- **The corollary for an estimate written in a roadmap entry**: the entry that says *cheap*
  should say what it is cheap *in*. R4's said neither, and two batches read it as cheap in
  milliseconds because the two rungs before it had been.

### Occupancy is a cost only `shaderstats` can show you (batch 59)

**Caching nine gathered `vec3`s in an `array<vec3<f32>, 9>` cost 0.147 ms at `cave`.** Removing
the array and evaluating the twelve corner curves inline -- **thirteen `light_curve` calls per
pixel rather than nine** -- was faster. The reason is one row of `shaderstats`: `resolve`'s shared
memory went **15,872 to 16,896 bytes**, and shared memory is what decides how many workgroups an
SM can hold at once.

- **A WGSL array indexed dynamically is a memory allocation, not a register file.** The obvious
  cheap-looking rewrite -- compute nine values once instead of thirteen times -- bought fewer
  instructions and lost residency, and residency won.
- **Batch 47 found the same shape from the other end** and it is worth holding the two together:
  pointing all nine gather reads at one cell, with perfect sharing as far as the memory system is
  concerned, recovered only a tenth of what removing them did. **This pass is instruction-bound
  and residency-bound and not memory-bound**, so trading arithmetic for residency is the trade
  that pays and trading memory traffic for arithmetic is not.
- **Read the shared-memory column, not just bytes and registers.** Three batches have quoted
  `shaderstats` for code size and register count. This is the first to need the fourth number,
  and it was the only one that moved.

### Spend a null result by widening, not by repeating (batch 58)

**Batch 55 measured three taps at the price of one and concluded the cost was the code existing
rather than the work.** That finding sat unspent for two batches. Batch 57 added a scalar field
and took one tap; batch 58 needed a *coloured* field beside it and did not add a second tap --
it put the bounce in the `.rgb` of the texel whose `.a` was already the occlusion. `resolve` went
**+128 bytes at 96 registers**, and both bench readings are inside their own error.

- **A "this does not scale" result is a licence to widen the thing that does not scale.** The
  obvious use of batch 55's number was "so a richer field is affordable"; the actual use was "so
  put the richer field in the same fetch". Those differ by a whole texture and a whole tap.
- **The corollary is a limit and it should be written down when the licence is spent**: the
  texel is now full. A seventh channel is a second texture and a second tap, and **batch 55's
  number does not cover that** -- it measured three taps into *one* texture, which is a
  different claim from one tap into each of two.
- **When a cost claim rests on a mechanism rather than on a resolvable delta, put a test on the
  mechanism.** `one_tap_serves_both_fields` asserts the shader takes exactly one sample and
  `shade_hit` calls it exactly once. Splitting it in two later would double the read cost and
  leave **every picture identical**, so no capture, no control and no `bitexact` row could catch
  it -- which is precisely the shape of thing that needs a source claim rather than a sweep.

### Say which identity you mean when a normalisation has two (batch 58)

**A draft of this batch's own prose claimed that reference ground bakes to 1.0 per channel, and
the arithmetic is three lines.** `albedo * luma(FLOOR_TINT) / (luma(albedo_ref) * FLOOR_TINT)` is
1 in **luminance** by construction and is 1 **per channel** only if the reference ground is grey.
Grass is not grey; it bakes to `(0.416, 1.259, 0.156)`.

Both identities are real and they are claims about different inputs:

- an **unbaked texel** returns 1.0 per channel, which is what makes the control bit-exact;
- **reference ground** returns 1.0 in luminance, which is what keeps the before/after pair
  rankable by making the feature's arrival a change of hue rather than of exposure.

**Writing the test is what separated them**, because a test has to name the input -- and the
first version of it asserted the wrong one against the right function and failed. **A comment
can hold two identities at once and a test cannot**, which is the argument for writing the
assert before the paragraph rather than after it.

### When the derivation admits more than you want to ship (batch 53)

**Say so, price the difference, and file it as a decision rather than as a bound.** Batch 53's
dark-water rung is derived from the sky flood: a tile whose rays are 15 blocks under the surface
is looking at content the sun never reached. The derivation permits a **shallow** camera to
trigger it by pitching down, which is true about the content and costs 55,058 pixels at max
delta 9 at a vantage the user had already made a decision about.

The reason to refuse it is not physics -- it is that the clamp would become a property of where
the player is *looking* rather than where they *are*, so the seam pops on a turn rather than
sliding on a swim. **That distinction is invisible to every still frame in the fixture**, which
is why it has to be written down at the gate and why the number it costs is on file in
`errors.md` rather than lost. A later batch, or the user, can take it in one line.

### An interval test needs both endpoints when which one binds depends on a sign

The same batch's first cut tested the far end only, which reads more natural and is wrong: a
descending ray is shallowest at the near end and a rising one at the far end, so one test admits
the other's failure case. **Whenever a quantity is linear in `t` and you are asserting something
across a range, check both ends unless you can name the sign.** The fixture caught it, at a
vantage that exists for a different feature -- which is the argument for running the whole sweep
rather than the vantage your change is aimed at.

### A ray's bill is that it was fired, not how far it marched (batch 102)

Batch 102a put three extra shadow rays per sun-facing pixel behind a careful budget and
found the budget was not the bill. Measured on hardware at the arm's most expensive
station (`--grass-dense`, 1920x1080, paired and alternated): the whole cone scaffolding
-- a second grid walk returning blocker distance, a perpendicular basis, the surface
hash, and the function inlined four times into `shade_hit` -- costs **0.073 ms**; the
first offset tap adds **1.824 ms** and the next two **1.33 ms each**; and cutting the
majority branch's march from 64 blocks to 8 -- an eightfold shortening -- saves **7%**.
The per-ray constant is launch, grid setup, the `march_chunk` entry and the divergence
it drags in, exactly the term batch 40's water-shadow bound was also paying. Three
consequences, bought with a failed arm:

- **Halving a ray feature's cost means firing fewer rays.** Shortening, narrowing and
  bounding are noise against the per-ray constant. So the *detector* that decides
  whether a ray fires must not itself be a ray -- batch 65's live-range lesson from the
  other side: the gate has to be cheaper than the 1.3 ms it saves, which arithmetic is
  and a walk is not. G5's boundary gate reads the blocker distance the centre tap
  already returned, for this reason.
- **Paying by effect is the whole design problem.** The arm moved 3.7% of the frame;
  the 96.3% that is deep umbra or open lit paid the whole bill and changed nothing. Had
  only the moved pixels paid, the arm priced at ~0.17 ms -- inside its +0.35 ms bar. A
  ray-priced look arm lives or dies on how well it finds its own pixels *before*
  spending the ray.
- **A symmetrical effect's acceptance statistic is the sign split, not the MAE.** A
  penumbra replaces a step with a ramp: ~half the moved pixels should lighten, half
  darken, signed mean near zero. This arm measured **86% lighter**, and neither the MAE
  ladder nor any crop had caught it -- "softens the edge" turned out to be "lifts the
  shadow", one numpy call late. Anything advertised as *redistributing* an existing
  quantity -- a blur, a ramp, a penumbra -- answers this question before its look
  verdict is heard.

## Planning a *constant* change

- **Measure the suspect before widening a constant something else reads.** Widening `BLEND` would
  have changed the terrain's shape to fix six numbers the terrain does not read.
- **A sweep that will not bottom out is fitting the wrong parameter.** A monotone sweep is not a
  hard problem, it is a misidentified one -- go and find the missing factor.
- **A default is a claim, and it is the only copy of the claim the program reads.** Wherever a
  constant is named for the build it *reproduces* rather than for what it *is*, go and look at
  what the default points at. Batch 26 was one line for exactly this.
- **Reverting a change reverts its code and not its prose.** When a batch abandons a fix, hunt the
  things that *describe* it -- a comment, a doc line, a test binding named `shipping`.
- **When a field is analytic, score the expression before you render it**, and first ask whether
  the statistic you want has a closed form at all. A negative result costs minutes; three
  documents once argued about an autocorrelation that never needed an estimator.
- **An optimiser will spend your proxy's blind spot.** An unconstrained search put two wave
  octaves at the same angle and scored better for it. The constraint that stops it has to be
  authored, asserted and priced.
- **A sweep that never moves is not a wide margin, it is a blind vantage** -- and it reads exactly
  like permission to ship the aggressive value. Batch 48 got 16 vantages, 0 pixels at every
  setting *and* at every extinction down to `--water-absorb 0.05`, then found the vantage's
  content ended inside the shortest value tried. **Push the constant past any plausible number
  until the frame moves; if it never does, you are measuring the camera.**
- **And a blind vantage is not a *conservative* one either, which is the half batch 48 got
  wrong in the other direction.** Unable to see what a shorter reach deleted, it shipped the
  safe value and wrote the crop up as the thing that would one day *stop* the aggressive one.
  Batch 52 built the crop and it licensed the aggressive one instead -- 128 blocks for 1.930 ms
  of a 10.6 ms frame, at a cost the fixture could finally show. **A measurement you cannot take
  does not argue for caution; it does not argue at all**, and a batch that reads "I could not
  see it" as "therefore be careful" is guessing with a confident tone. What a crop buys is not
  safety, it is the right to decide.
- **To find the vantage that can see a feature, build the version where the feature is
  maximal and diff against the shipping one.** Batch 52 needed a camera where a submerged
  distance cull had something to cull, and there is no way to reason that from terrain. A
  `WATER_FAR_DIST` of **0** culls everything the rule can ever reach, so ten seeds by eight
  yaws at 640x360 -- two renders and one `compare` each, six seconds a candidate -- ranked
  every camera in the world by how much the feature could possibly do there. The top six went
  to full resolution against the value actually under test. **The screen is the same trick as
  batch 47's nine-reads-one-address and batch 46's `shaderstats` pass: a cheap build that
  isolates one factor, run across every candidate before anything expensive runs on one.**
- **The constant's own comment is a source, and reading it is cheaper than a batch.** Batch 48's
  roadmap entry rested on "absorption kills the image within tens of blocks"; `WATER_EXT`'s
  comment three files away said red dies in a few blocks and *blue survives tens*, which is a
  twenty-fold difference and a different design. **Before pricing an entry, open the constant it
  is an argument about.**
- **Check which pass will read a new override before choosing that shape.** Batches 41 and 43
  made a tunable an `f32` in `SPEC_MASK` and it is the house pattern -- but only `march` and
  `resolve` get more than one pipeline, so a constant for any other pass has to be a plain WGSL
  `const` and its control a plain run-time flag.

## Planning a *method* batch

- **A measure that cannot see a known-positive is not fit to rule anything out**, and a fixture's
  known-positives can themselves be calibrated against a bug.
- **An instrument reports a number whether or not it is measuring anything.** A sweep that comes
  back suspiciously uniform has usually measured the sweep. **Put the assert in the tool, not in
  the plan** -- what caught the line-ending fiasco was an assert in the edit script, not any
  amount of rereading the output.
- **A no-op claim cannot be told apart from a not-wired-up claim.** Keep one in a pair with a
  positive claim, reading the quantity from two different places.
- **A filter you apply to a search is a claim about where the answer is not.** Items you excluded
  on purpose never appear in any output to look wrong. This batch's own narrative was written
  backwards because `docs/batch-21*` was filtered out of a consistency sweep as noise.
- **A verification vantage that shows nothing still counts itself.** When a sweep prints hashes,
  look for two rows that match.
- **A second implementation is affordable when the first can be made its tested twin.** The WGSL
  biome field is a transliteration of the function `WorldGen` actually calls; a transliteration
  source nothing uses would have rotted.

## Planning a batch the *user* found (batch 54)

**Fifty-three batches of measurement never found Snell's window, and one session of playing
did.** That is not a failure of the fixture; it is a statement about what a fixture is. Every
camera in the set was dry or three blocks down, and the defect needed a deep one -- so the
instrument could not have seen it, and no amount of sweeping would have changed that.

- **A defect reported from play arrives with its own vantage, and that is most of its value.**
  The screenshot carried a depth, an angle and a HUD. Before writing a line, reproduce the
  *geometry*: batch 54 computed the escape boundary at that depth (14.0 degrees), the critical
  angle (41.4), and the band between them, and that arithmetic said what the answer was **not**
  before any code was written. **The cheapest thing you can do with a bug report is check whether
  the mechanism you are about to reach for could even produce it.**
- **Say plainly when the user's diagnosis is right and your last batch was aimed elsewhere.**
  P4's depth sub-lead came from the same complaint -- *"it looks odd from inside when I go too
  deep"* -- and addressed the **cost** of deep water rather than the **look** of it. Filing that
  correction against your own shipped batch is worth more than the batch.
- **When they judge the build, their corrections may be one mechanism wearing two hats.** *"No
  mirror in the first three blocks"* and *"darker with depth"* read as two features and are the
  two ends of one ramp over the eye's depth. **Look for the shared quantity before building two
  things**; the version with one constant is the version that can be explained.
- **A look constant the user set by eye is recorded as theirs, with the sentence.** Not because
  of provenance for its own sake, but because the next session will find a number the physics
  does not support -- the critical angle is 48.6 degrees at *any* depth -- and needs to know it
  was a decision rather than an error.

### Two more arrived the same way, and the later one ranks the instruments (batches 62 and 64)

**Batch 54's Snell window, batch 62's LOD shadow pop and batch 64's yaw-dependent shadow were all
found by playing; the last two both live in `grid`, and no vantage in the fixture could have shown
either.** Three of them is a pattern rather than an anecdote. What batch 64 adds is a ranking of
what to reach for once the report arrives.

- **An MAE cannot rank a pop, and that is the sentence missing from batch 63's lesson.** That
  batch established that a pixel count cannot rank a *look* -- 759,140 pixels at MAE 1.22,
  invisible in a slider. Batch 64's whole frame is **MAE 0.2552** and the defect is a shadow
  *switching* between two frames, which no single frame can hold at all. **Both statistics are
  computed over one image, and a discontinuity is not in one image.** Check what the defect is a
  property *of* before choosing what to measure.
- **Choosing a vantage is choosing a metric, and the two can rank cameras opposite ways.** Two
  candidate cameras for the new vantage: `--time 0.28` moves **32,937** pixels for a crop MAE of
  **0.22 at max delta 9**, `--time 0.32` moves **18,468** for **2.6165 at max delta 27**. A
  count picks the first and an eye picks the second. The `CHECKS` row that guards the vantage is
  therefore an MAE and not a count, because a count would pass on a build whose shadow had gone
  faint.
- **State the claim as an equality somewhere a capture cannot reach.** `tests/shadow_grid.rs`
  asserts the grid-eligible set is identical at eight yaws from one position, and that the drawn
  set is not. It runs in 1.4 seconds against a real streamed world, needs no device, and says
  what the 18,468 pixels were supposed to move *to* -- which the pixel count cannot.
- **A number wearing a known fault's signature is eleven commands from being told apart from
  it.** `open-sea` moved **1 pixel at max delta 1**, which is exactly roadmap D1's intermittent
  bistable pixel. A-against-A came back clean **8 of 8** and the A/B reproduced it **3 of 3**, so
  it is the feature. The cost of not checking is a true measurement filed as a known fault, which
  is the inverse of batch 57's error and just as durable.

### Decoupling two consumers of one list is free when the order already encodes the split (batch 64)

**The entry said this was a batch rather than a one-line fix because `visible` fed two consumers
with different needs**, and that framing was right while the implied cost was not. `tile_select`
box-tests every entry per tile and must stay frustum-culled; `grid` must not be. The two were
decoupled by **concatenation** rather than by a second array, a second buffer or a second
uniform:

- the index into the chunk array **is** the chunk id, so the frustum-culled entries go first;
- `frame.chunk_count` keeps the length of that prefix, and `tile_select` is the **only** reader
  of that field anywhere, looping `0 .. chunk_count`;
- `grid` reaches the tail by index and carries no count, so it needs nothing.

Measured: `tile_select` **+0.000 to +0.001 ms at three vantages in both Hi-Z arms**, with the
array more than doubled (157 entries to 346 at `default`). **Before adding a field to `GpuFrame`
or a second binding, check whether one existing count can be reinterpreted as a boundary** --
here it already was one, and nothing but a comment had ever said so.

**And bound the new set by something structural rather than by a distance.** The tail is every
resident chunk that covers at least one cell of the existing 32x8x32 lattice, because a chunk
covering none of it can never be named by a grid cell -- so the batch adds **no constant**, and
the bound cannot drift out of agreement with what reads it. `grid_span` is one function with two
callers for the same reason: the filter that admits a chunk and the loop that writes its cells
cannot disagree if they are the same code, and the disagreement that costs something -- the
filter dropping a chunk the loop would have written -- is this batch's own defect one level down,
invisible in exactly the same way.

## Census tests earn their keep on the batch that adds a caller (batch 54)

**Three existing tests failed on this batch and every one of them was right.** `trace_world`'s
caller census, `shade_hit`'s light-model census, and `SPEC_WATER_SHADOW_DIST`'s *uniqueness*.
None of them checked a value; all three checked a **count**, and the count is the mechanism: a
new transport cannot be added without someone deciding its `skip_water` policy and its light
model, because the build stops until they do.

**And then all three were reverted, which is the ending worth having.** The transport they were
objecting to -- a traced mirror on the underside of the water surface -- turned out to be worth a
max channel delta of 3 and to cost 4 ms, so it was removed and the censuses went back to their
original numbers. **The tests did their job twice**: once by making the new caller declare
itself, and once by returning to exactly the claim they made before, with nothing to unpick.
That is what a census buys over a policy assertion scattered across call sites.

- **Write the census, not the policy, when the policy is per-caller.** A test that asserted "the
  refracted leg passes `true`" would have passed silently through a fifth caller that passed
  whatever it liked.
- **A uniqueness claim can quietly become an unreachability claim -- and the row you add to
  prove it may simply be false.** `--no-water-refract` clears `--no-water-shadow-cut` at four dry
  vantages. With a second reader added, that stopped being a fact about the code and became a
  fact about the cameras, so the batch added a `deep-water` row asserting the opposite. **It
  failed.** `--no-water-shadow-cut` moves **0 pixels at all three submerged vantages**, with the
  second reader present or absent, because a mirrored hit is open sea floor at range with no
  occluder in the band and heavy absorption over the return. The honest record is *reachable and
  invisible*, which is weaker than either thing anybody wanted to write -- **write the weaker
  thing.** (The reader was then removed for unrelated reasons and the question went away, which
  is luck and not method.)
- **A prose mention can break a census, and that is the census working.** One comment naming the
  light-model gate inflated its count by one. The comment was reworded rather than the test
  loosened, because a test that ignores comments cannot tell a mention from a call.

### And the inverse: a census that transcribes a table's size goes stale the day it grows (batch 59)

**Three existing tests failed on this batch and none of them was right.** All three had a literal
where they should have had a derivation, and all three broke because the *table* grew rather than
because anything they were guarding had changed:

- `tests/edit_journal.rs` refused `--bar 20,...` as "no such block". Id 20 became `AMBER_LAMP`, so
  the assertion became *a perfectly legal bar is refused*. Now `format!("{}", block::BLOCK_COUNT)`.
- `tests/repeat.rs` asserted `none.len() == 13`, the count of layers at permutation class `NONE`.
  Three new atlas layers made it 16 -- a test whose subject is the *permutation* table failing
  because the *atlas* grew, which says nothing about either. Now `tex::COUNT - 8`, which pins the
  claim actually worth pinning: one `FLIP_U` and seven `D4`.
- `tests/textures.rs` hashed the **whole atlas** as batch 21b's control, to say the mottle changed
  nothing else. Appending layers changes that hash. Now it hashes the 21 layers that existed when
  the constant was measured, which keeps the claim and drops the objection to appending.

**The rule is the same one the section above makes, read backwards.** A census is worth having
because it counts a *mechanism*; a census that counts the *size of a table* is a transcription,
and `block::tex` ids and block ids are appended and never inserted, so growth is the one edit
those tests must not object to. **If the number in an assertion would change when something is
appended, derive it from the thing being appended to.**

### And a bug this batch shipped and caught, which is the control-table failure in miniature

The first build gated the new term's **traced mirror** on its control flag and left the Fresnel
mix outside the gate. `--no-snell` then reverted the *detail* of the mirror rather than the
mirror: the frame moved by a max channel delta of 3 where it should have moved by 136, every
source test passed, and the only thing that could have caught it is a `bitexact` row failing for
a reason no image explains. **A control's gate belongs at the top of the thing it names, and it
has to return the caller's input untouched** -- which is also what makes "bit-exact" mean
bit-exact rather than close.


### Three rounds, three counts: a census failing is the test naming the pattern you missed (batch 90)

**Round 1 and round 2 taught the code their mechanism; round 3 taught the *tests* theirs.**
Batches 81b/81c, then 90, each broke a different census the same way: the count of a *set* was
updated in one file and not the sibling file that pinned it a second time. `SPEC_HI_MASK`'s
literal 6 lasted two batches past the batch that made it 8. The user's diagnosis is the one to
keep: **these tests assert a *number* where they mean a *property*.** So batch 90b converted the
class instead of re-pinning it: `tests/spec_hi.rs` asserts, for the whole FLAG_HI word, that
every constant is one bit, every pair is distinct, every bit is in the mask, and the mask has no
anonymous bit -- with the enumeration (`ALL_HI`) sitting beside the assertions as the **one**
line an added constant has to grow. Nothing else in the tree re-pins the word's size.

- **The single-enumeration-point is the whole trick.** A count in two files fails in the wrong
  file; a list in one file fails in the right one.
- **"The mask is exactly the union of the named bits" is the fourth property on purpose** --
  without it, an anonymous bit slips through all three others, which is the same defect class
  from the inside.

### "Built" has a pixel threshold: a flagged arm that measures zero is not built (P-A, A9)

Two features in one tree now compile, ship dark behind flags, pass their tests, and measure
zero: batch P-A's f16 re-pack (80 registers to 80 -- the allocator rematerialized the span and
nothing it was built to move moved) and batch 90 A9's caustics (115 of 921,600 pixels at max
delta 1 at its best vantage, zero at its prescribed ones). **Both are filed as
*diagnosed/priced but NOT built*** -- which sounds pedantic until the third one lands and the
trend is real. The standing acceptance line, costing one `bitexact` run, for any arm whose
build is conditional on its evidence:

> **A new arm has to move pixels at a named vantage by more than max delta 1 before it counts
> as built.** "The override compiled" is a sentence about plumbing; it is never the claim.

A9's mechanism is diagnosed rather than guessed at, in [roadmap A9](roadmap.md) and
[water.md](water.md): the arm ran (the shallow-water 115 pixels prove the gate fires), and its
*carrier* is the defect -- a product of two slope-squares is quartic in `wave_amp`, evaluating
to at most 1.1% of direct sun at shipping defaults, where any gain that fixes it explodes
sixteen-fold for a doubling of amplitude. The wave trains are also the wrong carrier for the
world's scale outright (lambda 32 blocks against a caustic's meter-scale ribs), gain aside.


## Two entries filed as readings were experiments, and both ran in minutes (2026-09-15)

**Neither of the front-of-queue entries needed a session and both had their own test written into
them.** One had been open since batch 57 and one since batch 59. The batch-59 one took three
builds and twelve minutes; the batch-57 one took twenty repeats of a command the fixture already
had. **What kept them open was the filing, not the difficulty** -- `CLAUDE.md`'s rule sends an
unexplained number to the front of the roadmap so nobody chases it mid-batch, and the cost of
that rule is that the entry then reads like a mystery rather than like a queued experiment.
**Write the entry so its first line is the command**, and the next session can clear it before
starting.

- **A zero sampled once from an intermittent process is not a zero.** The batch-57 entry ruled
  out *"it predates batch 57"* on **one** run against `voxelcraft-pre57.exe` reading 0 pixels.
  The phenomenon fires about 1 run in 15, so that single clean run carried essentially no
  information, and the file asserted it for two batches. Re-run 40 times it came back clean 40/40
  -- the same conclusion, now with evidence. **Before a zero rules anything out, ask how often
  the thing you are ruling out would have shown up in the number of samples you took.** This is
  [`harness.md`](harness.md)'s *"a zero is only as meaningful as the frame's ability to show
  one"* with the frame replaced by the sample count, and it is the more common of the two.
- **When two configurations provably compile to the same thing, the question is not about
  either of them.** The batch-57 entry had already written down that both sides of the failing
  claim carry the same flag word and *"cannot legitimately differ at all"*, and then went on
  suspecting the claim for three batches. That sentence makes A and B the **same command**, which
  turns the whole question into *is one render reproducible* -- an A-against-A test, which found
  it in 30 runs. **A proof that a difference is impossible is a redirection, not a dead end.**
- **Ask what a gate costs before crediting it with what it skips.** Batch 59's gather gate costs
  **0.351 ms** at `cave` to save **0.678**, at identical registers and shared memory, with a
  *uniform-false* branch. The arm that looked slower on an instruction count was faster because
  it had no data-dependent branch in it. **On this pass a branch is not bookkeeping around the
  work; it is a comparable fraction of the work** -- which is the same shape as batch 59's own
  finding that occupancy beats four `light_curve` calls, one level down.
- **A diagnostic whose picture is provably identical is better than one whose picture is thrown
  away.** Batch 53's method is *patch a literal to a wrong value, count, discard the frame*. Here
  both patched literals happen to render **bit-exact** with the shipping build at the vantage
  being benched, because the data makes the branch's two arms agree there. That upgrades the
  reading from "two numbers about different pictures" to a pure timing A/B, and it is worth
  **checking for** before assuming the picture has to be wrong: run `bitexact` on the diagnostic
  first, it costs one command.

## Ask what share of the frame's light a rung can reach, before asking which resource pays (batch 60)

**`CLAUDE.md`'s scarcity table teaches you to ask which resource pays, and batch 60 got a clean
pass on that question while nearly wasting the batch.** The rung is free on the frame and costs
1.24x of a bake nobody is waiting on -- textbook. It is also aimed at the **ambient floor**, and
`Config::ambient` is 0.08, so the floor is about 7% of the light on a lit surface. **No accuracy
improvement to a 7% term can be worth more than 7%**, and that was knowable before a line was
written: [`ADR 0001`](adr/0001-no-elevation-form-factor.md) had measured the same ceiling one
batch earlier and had been read, cited, and then not applied to the new rung.

The number that says so: the transport half alone is **MAE 0.372 at `default`**, where the term
ADR 0001 rejected *by eye* was **2.87**. Eight times under something already judged invisible.

- **Two questions, and this file had only written down the second.** *What share of the frame's
  light can this rung reach?* comes first; *which resource pays for it?* comes second and is
  only interesting once the first has a good answer. Batch 60 passed the second and would have
  failed the first, and was saved by adding energy rather than by the transport work.
- **A ceiling in an ADR is about the quantity, not about the implementation that hit it.** ADR
  0001 reads as a verdict on the elevation form factor. It is actually a verdict on
  `floor_rgb`, and it binds every future term that multiplies it -- which was not obvious from
  where it is filed. **When an ADR rejects something on a budget, go and check whether your own
  entry spends the same budget.**
- **The corollary, and it is why R5 is next**: the sequence's remaining energy is in the direct
  term and the nine-cell gather, not in the floor. *Iterating a quantity worth 7% converges to
  something worth 7%.*

### Share of the light and share of the pixels are two questions (batch 61)

**Batch 60 added *ask what share of the frame's light a rung can reach* to this file, and batch 61
is the rung that passed it and failed anyway.** Roadmap R5 aimed at the **direct** term -- the
largest in the frame, and precisely where batch 60's corollary said the remaining energy was. It
was built in full, measured at **+2.639 ms of a 6.165 ms `resolve`**, and reverted, because it
moved **471 pixels of 921,600 at max delta 6**. [`ADR 0002`](adr/0002-no-disc-sampled-sun.md).

The gap between the two questions is the whole lesson: **a term can carry most of the frame's
light and still be *modified* almost nowhere.** A soft shadow changes only the pixels at a
shadow's edge. The direct term is enormous; its edges are a few thousand pixels.

- **Ask both, in this order.** What share of the light does the rung reach, and then *what share
  of the pixels does the change reach*? R1, R2 and R3 all pass the second easily -- they modulate
  a term everywhere it appears. R5 does not, and nothing about its energy could rescue it.
- **The quantum of the world bounds the effect, and that is checkable on paper.** Two rays across
  the sun's disc diverge by `2 * 0.00465 * d`, which reaches one voxel at **d = 108 blocks**. Below
  that every ray walks identical cells and the shadow is *exactly* hard. One division would have
  ranked this entry before a line was written, the way `Config::ambient`'s 0.08 would have ranked
  batch 60's.
- **The cheapest check of all was of the premise, not the effect.** R5 proposed storing the
  horizon `probe.rs` already computes and discards. Ten minutes of reading found `shaft.rs`
  computing a *better* horizon every frame -- finer texels, the exact sun azimuth, five times the
  reach, already soft. **Before storing a derived field, grep for a pass that already derives it.**
- **And budget a ray count superlinearly.** Four taps measured **6.11x** one, not 4x: `resolve`
  is 5.649 ms with no sun shadow at all, one march is 0.516 and four are **3.155**, which is
  **1.53x** the linear prediction. Rays a fraction of a degree apart still diverge enough through
  the tree to cost more than their count. **And subtract the baseline before taking a ratio** --
  a draft of this entry divided the paired *delta* by one march, got 5.1, and quoted it as the
  ratio; it is the cost of the three *extra* taps in units of the first.

### The ceiling question asked *before* building, for once -- and what it caught (batch 65)

**The two questions above were both added after the fact. Batch 65 is the first rung that asked
them first, and the answer is the whole reason it was worth opening.** R3's rung modulated a floor
`Config::ambient` pins at ~7% of a lit surface; R7's carried `SKY_TINT`'s luminance by
construction, so only hue could move. Both found their cap *after* being built. A specular
reflection off a dielectric is neither -- it is added outside `albedo`, and is not a share of the
ambient -- and Fresnel bounds it instead: `GROUND_F0`'s 4% at normal incidence, but **20% at
cos 0.3 and 61% at cos 0.1**, on ground that is at grazing incidence over most of its extent. It
shipped at **MAE 5.98**, the loudest rung in the sequence.

- **"What caps this?" is answerable on paper, and the answer is the entry's whole ranking.** Three
  rungs in a row had a cap and two of them found it by building. **Write the cap in the entry.**
- **Reach for the curve's shape before its scale.** R6's first build had no roughness term and came
  back a blue-white haze -- **MAE 15.65, max delta 202** -- because Schlick reaches 1.0 at grazing
  for *every* F0, and what a bare Fresnel is missing is the geometric shadowing-masking term: the
  microfacets that would mirror a grazing ray are occluded by their neighbours at exactly the angle
  Schlick says reflectance is highest. **A gain would not have fixed it**, and that is measured
  rather than argued -- the roughness ladder from 0.80 to 0.97 moved MAE only 11.50 to 10.35, so
  the grazing term was never the bulk of the error. **When a ladder over a constant barely moves
  the metric, the fault is the function and not the number.**

### An identity chosen to make absence exact will read as presence somewhere else (batch 60)

**The one real defect in batch 60, and it is the field's central design decision read
backwards.** Batches 57 and 58 store occlusion and bounce **normalised to 1 in an open field**,
so that an unbaked texel, a coarse LOD and a pinned field all render the previous build *to the
bit*. That is a good decision and it is why three rungs in a row have exact controls.

The trap is that **1.0 means "unoccluded", and it is also what the bake returns where it has no
opinion at all**. `probe::bake` reads `WorldGen::height` and never the voxels, so it has no
opinion about anything underground. A new term that gated on `probe.a` as *"how much sky is
here"* therefore read a sealed cave as **fully exposed** and lit it with bounced sunlight:
**921,600 pixels at `cave` and at `lamps`**, on a build whose every other number looked right.

- **Ask what a sentinel means when the answer is unknown, not only when it is zero.** An
  identity picked so that *absence of the feature* is exact says nothing about what *absence of
  data* should mean, and the two are the same value.
- **The right quantity was already in scope two lines up.** `sky_sm` is the flood's own
  per-voxel answer, is zero in a cave and a sealed room, and already carries `frame.daylight`.
  **Prefer the term the pass already computed for the same question** over a second source that
  happens to be in the same texel.
- **The confirmation was better than the fix and cost one command.** `cave` reads **0 pixels
  between the two gain settings**, so no value of the constant can leak underground -- a check
  that does not mention the bug, and would have caught it.

## Verifying

- **Check that your own numbers have the shape of the inference you are borrowing (batch 59).**
  That batch wrote, into two documents, that its cost *"tracks surface density -- 0.321 at `cave`,
  0.340 at `default`, 0.150 at `sky`, which is batch 56's ordering and says the term is work
  rather than code."* Batch 56's ordering has `cave` clearly ahead of `default`, and **that
  ordering is the entire argument**. Here `cave` is 0.321 +/- 0.032 against `default`'s 0.340 +/-
  0.049 -- indistinguishable, and in the wrong direction if anything. The inference was borrowed
  intact from a batch whose numbers had the shape it needs, onto numbers that do not.
  **A precedent is a template for an argument, not a conclusion you inherit**; re-derive it
  against the readings in front of you, including the error bars, before quoting the batch it came
  from. The user caught this by asking what `cave` actually looks like.
- **A fix nothing exercises is not a fix.** Batch 9's tree gate was fixed exactly as planned and
  guarded nothing, because no biome had both a non-grass surface and trees. Giving the desert
  palms is what made it load-bearing.
- **A feature whose whole purpose is what happens when the ordinary path does *not* run needs a
  flag that stops the ordinary path.** `--edits` writes the same file on a clean exit whether it
  appended as it went or not, so batch 33's crash safety was unobservable until `--no-exit-save`
  existed. Build that flag with the feature, not after it, and keep it in a pair -- a no-op claim
  alone cannot be told from one that was never wired up.
- **When a batch relaxes a check, scope the relaxation with something on disk rather than with
  which code path is reading.** Batch 33's append log has to be allowed a file longer than its
  header claims, and the old strict check is what catches a truncated save. A flag *bit in the
  file* separates them; "the loader knows who called it" would not have, and the check every
  earlier A/B depends on would have been quietly gone.
- **Coverage is the wrong statistic for a gate; a rate normalised by its own scale is the right
  one.** A bald desert moves the world's canopy fraction by hundredths of a per cent and reads
  exactly zero on trunks-per-eligible-column.
- **A harness can be blind to the thing you are pricing and will not say so.** `--bench-terrain`
  generates LOD 0 only, so batch 18's bench ran clean and identical with the feature on and off --
  not because it was free but because the pass never executed.
- **Change one thing per build, including how the thing is sampled.** A test that changed the uv
  *and* the mip proved only "not this layer as currently sampled", and cost three builds.
- **When a change that must move the image moves nothing, believe the null result.** It is the
  frame telling you it never read that code.
- **Assert against a constant the program already has, not against a number you typed.** Batch
  35 asserted its scan matched a brute-force gather to `1e-3`, on the strength of a theorem that
  holds over an integer lattice and not over the bilinear taps the code actually makes. The test
  failed, the code was right, and **what it caught was the batch's own prose in three files**.
  Rebinding the bound to `shaft::SOFT` -- the ramp that decides whether a disagreement can move
  a pixel at all -- turned an arbitrary epsilon into a claim that stays true when the constant
  moves.
- **A test whose premise is "by construction" is only as good as the construction.** The same
  batch asserted a cliff top could not shadow itself because nothing upwind is higher. Nothing
  upwind of the *steepest step* is higher was never true -- a mountain goes on rising behind it.
  The column that genuinely cannot be shadowed is the window's global maximum, and that one is
  by construction.
- **Bit-exact and free are two claims, and a control earns the second only by construction.** If a
  control has to be free, the term it disables has to be reachable from one place.
- **A lint that is right in general can be wrong where the formulation *is* the documentation.**
  Three of clippy's suggestions here would each have cost real meaning. Take the rest -- a silent
  baseline is what makes the next warning visible -- but never run `--fix` blind.


## Working with the documentation (2026-09-18)

- **Deleting a document without mapping its references breaks the codebase's memory, not its build.** The 2026-09-18 cleanup removed the documents `src/` points at by name (`docs/ledger.md`, `docs/harness.md`, ...); the build stayed green while the comments quietly started pointing at nothing. Grep the tree for what references a document before deleting it.
- **The link guard is a test, and it dies with the documents it guards.** `tests/docs.rs` exists so a dead `](path)` in any `.md` fails the suite, and the guard encodes more than links: the briefing must keep naming the five split-out files, no document may transcribe a test count, and the README may neither quote a measurement nor grow a flag list. When documents are removed or restructured, the guard is re-run in the same session — a ledger line that quoted a raw test count once survived its own guard because the suite had last run before the rewrite.
- **Check the remote before pushing a local branch.** The documentation cleanup landed twice: once in this checkout, once already on the remote branch, one file ahead. The origin refspec tracks `main` only, so fetch the actual ref (`git fetch origin <branch>:refs/remotes/origin/<branch>`) before pushing; a rejected push with work on both ends is the price of not doing that.
