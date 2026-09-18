//! The measurement fixture: one place that says what a vantage is, what a crop is, what a
//! metric means, and how a reference is built.
//!
//! Batch 22's whole content. It renders nothing itself -- it drives `voxelcraft.exe` by path,
//! which is deliberate and not a convenience: the bit-exactness sweep every batch since 8 has
//! hand-rolled compares **two executables**, and batch 14 is the standing proof that a flag
//! which switches a feature off is not a build without the feature in it. A fixture that could
//! only measure itself could not do the one job the ledger most depends on.
//!
//! The driver is [`crate::bin::harness`] -- `cargo run --release --bin harness`.

pub mod bench;
pub mod metric;
pub mod vantage;

pub use metric::{Img, Rect};
pub use vantage::{Content, Crop, Vantage, VANTAGES};

use std::path::{Path, PathBuf};
use std::process::Command;

/// How a capture is taken, as opposed to where it is taken from.
#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    pub width: u32,
    pub height: u32,
    /// Off by default. The temporal pass hides high-frequency residual, which is the thing
    /// the look-metrics exist to measure.
    pub taa: bool,
}

impl Default for Recipe {
    fn default() -> Self {
        Recipe {
            width: vantage::STD_WIDTH,
            height: vantage::STD_HEIGHT,
            taa: false,
        }
    }
}

impl Recipe {
    pub fn args(&self) -> Vec<String> {
        let mut a = vec![
            "--width".into(),
            self.width.to_string(),
            "--height".into(),
            self.height.to_string(),
        ];
        if !self.taa {
            a.push("--no-taa".into());
        }
        a
    }
}

pub struct Capture {
    pub path: PathBuf,
    pub stdout: String,
    /// True when the child printed the `MAX_PAIRS` truncation warning. A capture that
    /// truncated is missing geometry and must not be scored -- batch 19 scored three of them
    /// before anyone noticed, because the flat blue that came back read perfectly plausibly as
    /// "very smooth water".
    pub truncated: bool,
}

/// Render one vantage to `out`, returning what the child said about it.
pub fn render(
    exe: &Path,
    out: &Path,
    v: &Vantage,
    recipe: &Recipe,
    extra: &[String],
) -> Result<Capture, String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--screenshot").arg(out);
    cmd.args(recipe.args());
    cmd.args(v.args);
    cmd.args(extra);
    let o = cmd
        .output()
        .map_err(|e| format!("running {}: {e}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    if !o.status.success() {
        return Err(format!(
            "{} exited {}: {}",
            exe.display(),
            o.status,
            stderr.trim()
        ));
    }
    if !out.exists() {
        return Err(format!("{} wrote no file", exe.display()));
    }
    // An ignored argument is an error here and not a warning. The parser drops what it
    // cannot read and exits 0, so this capture was taken with fewer flags than the
    // caller asked for -- which in a sweep whose pass condition is `0 pixels differing`
    // is indistinguishable from the claim being true. Fail loudly instead.
    if stderr.contains(UNKNOWN_ARG_MARK) {
        let ignored: Vec<&str> = stderr
            .lines()
            .filter(|l| l.contains(UNKNOWN_ARG_MARK))
            .map(str::trim)
            .collect();
        return Err(format!(
            "{} ignored an argument, so this capture is not the one that was asked \
             for: {}",
            exe.display(),
            ignored.join("; ")
        ));
    }
    Ok(Capture {
        truncated: stderr.contains(TRUNCATION_MARK) || stdout.contains(TRUNCATION_MARK),
        path: out.to_path_buf(),
        stdout,
    })
}

/// The phrase the renderer prints when `tile_select` asked for more (tile, chunk) pairs than
/// the buffer holds. One definition, shared with the code that prints it, so the fixture
/// cannot go deaf by having the message reworded underneath it.
pub use crate::render::TRUNCATION_MARK;

/// The phrase the argument parser prints for a flag it does not recognise. Same
/// arrangement and same purpose as [`TRUNCATION_MARK`]; see its definition for why an
/// ignored argument is the most dangerous thing that can happen to a bit-exactness
/// claim.
pub use crate::config::UNKNOWN_ARG_MARK;

// ------------------------------------------------------------ composition of the controls

/// What adding `also` to `base` is supposed to do to the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Composes {
    /// Nothing at all. `base` already implies `also`, so naming it a second time is a no-op
    /// and the two captures must be byte-identical.
    Nothing,
    /// Something. The two flags are independent, and this is the direction of the claim that
    /// keeps the other direction honest -- without it a check that had gone blind, or a flag
    /// the parser silently dropped, would read as a table of clean passes.
    Something,
}

/// One claim of the form "this control already clears that one", taken off CLAUDE.md's
/// control table.
///
/// **These are same-binary A/Bs, and that is why they are worth having.** Most of the control
/// table's bit-exactness claims are against a *revert build* -- "reproduces the pre-batch-9
/// rule set exactly" -- and the reverts for batches 1 through 20 do not exist in this tree and
/// cannot be rebuilt, there being no `.git` here. What survives is the half of each claim that
/// this build can check on its own: the composition. `--no-biomes` says it clears the tint,
/// the foliage and the meadow, and whether it does is a fact about *this* executable.
///
/// The rules being checked live in `flags_from` and in `worldgen::has_foliage` /
/// `has_meadow`, so reading the source answers the flag half. It does not answer the frame
/// half: `--no-water` and `--no-biomes` change what the *generator* emits as well as what the
/// shader is compiled with, and a composition can hold in the flag word while the two worlds
/// differ. Only a render settles it.
pub struct Implication {
    /// Side A's arguments.
    pub base: &'static [&'static str],
    /// What side B adds to them.
    pub also: &'static [&'static str],
    /// Which vantages can see this claim, comma-separated. Not "all of them": a claim about
    /// the wave field is unobservable on a dry vantage, and a table of zeros taken where the
    /// feature cannot appear is the vacuous pass this whole file exists to refuse.
    pub only: &'static str,
    pub expect: Composes,
    pub about: &'static str,
}

pub const IMPLIES: &[Implication] = &[
    // -------------------------------------------------------------- `--no-biomes` clears three
    Implication {
        base: &["--no-biomes"],
        also: &["--no-tint"],
        only: "default,coastline,lod",
        expect: Composes::Nothing,
        about: "`FLAG_TINT` is `cfg.tint && cfg.biomes`, so a one-biome world carries no tint \
                to switch off. This is what keeps `--no-biomes` a whole-frame control rather \
                than a terrain-only one",
    },
    Implication {
        base: &["--no-biomes"],
        also: &["--no-foliage"],
        only: "default,coastline,lod",
        expect: Composes::Nothing,
        about: "`worldgen::has_foliage` is `foliage && biomes`, and batch 15 folds the result \
                into both `march` and `resolve`, so the tufts are absent from the world and \
                their code absent from the module",
    },
    Implication {
        base: &["--no-biomes"],
        also: &["--no-meadow"],
        only: "default,lod",
        expect: Composes::Nothing,
        about: "`has_meadow` chains through `has_foliage`: a world with no ground cover has no \
                LOD boundary for the coarse paint to compensate for. `lod` is in the set \
                because it is the only vantage that puts a meaningful share of the frame in a \
                transition band, which is the only place a meadow claim can be seen at all",
    },
    Implication {
        base: &["--no-biomes"],
        also: &["--no-tint", "--no-foliage", "--no-meadow"],
        only: "default,coastline,lod,terraces",
        expect: Composes::Nothing,
        about: "all three at once, which is the claim the control table actually makes: that \
                `--no-biomes` is the pre-batch-9 rule set and not merely the pre-batch-9 \
                terrain",
    },
    // -------------------------------------------------------------- `--no-foliage` clears the meadow
    Implication {
        base: &["--no-foliage"],
        also: &["--no-meadow"],
        only: "default,lod",
        expect: Composes::Nothing,
        about: "the inner link of the same chain, and the one that does not depend on biomes \
                being off",
    },
    // -------------------------------------------------------------- batch 60's gain cannot leak
    Implication {
        base: &["--no-probe-sun"],
        also: &["--probe-sun-high"],
        only: "default,terraces,cave,night",
        expect: Composes::Nothing,
        about: "the louder gain is reachable only through the term it scales, so with that term                 off it must change nothing. **Worth a claim because the two sides compile                 different pipelines** -- `FLAG_PROBE_SUN_HIGH` is in `SPEC_MASK`, so side B                 keys a second `resolve` in which the override is set and the block it feeds is                 dead-stripped anyway. That is exactly the shape batch 58 found an ULP of                 reassociation in, and the claim is here to say whether this one has the same                 problem or is genuinely exact",
    },
    // -------------------------------------------------------------- `--no-water` clears the field
    Implication {
        base: &["--no-water"],
        also: &["--no-waves"],
        only: "coastline,open-sea,shore,lattice,terraces",
        expect: Composes::Nothing,
        about: "`FLAG_WAVES` needs `sea_level > 0`, which `--no-water` zeroes, so a dry world \
                cannot reach the wave field. Checked at all five water vantages because this \
                is the claim that a dry world is not a second world",
    },
    Implication {
        base: &["--no-water"],
        also: &["--no-waves", "--no-wave-aniso", "--no-wave-shoal", "--no-wave-fill"],
        only: "coastline,open-sea,shore,lattice,terraces",
        expect: Composes::Nothing,
        about: "the whole wave subtree at once, since all three of batches 19, 20 and 25 nest \
                inside `FLAG_WAVES` rather than sitting beside it",
    },
    // -------------------------------------------------------------- `--no-waves` clears its two children
    Implication {
        base: &["--no-waves"],
        also: &["--no-wave-aniso"],
        only: "coastline,open-sea,lattice",
        expect: Composes::Nothing,
        about: "a flat sea has no field to band-limit. `flags_from` nests this deliberately, \
                so that `SPEC_WAVE_ANISO` cannot become a live pipeline key selecting between \
                two identical shaders",
    },
    Implication {
        base: &["--no-waves"],
        also: &["--no-wave-shoal"],
        only: "coastline,open-sea,shore",
        expect: Composes::Nothing,
        about: "and no amplitude for the depth ramp to scale. `shore` is in the set because \
                its two-block lagoon is the only water batch 20 does any work for",
    },
    Implication {
        base: &["--no-waves"],
        also: &["--no-wave-fill"],
        only: "coastline,open-sea,lattice",
        expect: Composes::Nothing,
        about: "batch 25's eight octaves are still octaves of a field `--no-waves` removes, \
                and the nesting in `flags_from` is what keeps `SPEC_WAVE_FILL` from becoming \
                a pipeline key that selects between two identical flat seas",
    },
    Implication {
        base: &["--no-waves"],
        also: &["--no-wave-aniso", "--no-wave-shoal", "--no-wave-fill"],
        only: "coastline,open-sea,shore,lattice",
        expect: Composes::Nothing,
        about: "all three children together",
    },
    // -------------------------------------------------------------- and they are siblings
    Implication {
        base: &["--no-wave-aniso"],
        also: &["--no-wave-fill"],
        only: "open-sea,lattice",
        expect: Composes::Something,
        about: "the direction that keeps the four above honest. Batches 19 and 25 are \
                independent -- one filters the field and the other decides how many octaves \
                it has -- so switching off the band limit must not absorb the schedule. A \
                table of `Nothing`s with no `Something` in it is what a parser silently \
                dropping an argument looks like",
    },
    // -------------------------------------------------------------- the shoreline half (batch 75)
    Implication {
        base: &["--no-water"],
        also: &["--no-shore-wet", "--no-shore-foam"],
        only: "coastline,open-sea,shore,lattice,terraces",
        expect: Composes::Nothing,
        about: "`FLAG_HI_SHORE_WET` and `FLAG_HI_SHORE_FOAM` both need `sea_level > 0`, which \
                `--no-water` zeroes, so the dry world cannot reach either -- the same \
                claim `FLAG_WAVES`' row two up makes, one nesting later",
    },
    Implication {
        base: &["--no-shore-wet"],
        also: &["--no-shore-foam"],
        only: "coastline,shore",
        expect: Composes::Something,
        about: "and the direction that keeps it honest. The band is an albedo on sand and \
                the foam a term on the refracted column -- two terms, two materials, no \
                shared frame by construction -- so switching one off must not absorb the other",
    },
    // -------------------------------------------------------------- the god-ray asymmetry
    Implication {
        base: &["--cloud-shadow", "0", "--no-terrain-shafts"],
        also: &["--godray-strength", "0"],
        only: "default,low-sun,sky,coastline",
        expect: Composes::Nothing,
        about: "the control table used to call `--cloud-shadow 0` the deeper of the two and \
                say it reproduced the pre-batch-11 build *on its own*, because the deck was \
                the shaft's only occluder. **Batch 35 gave the shaft a second occluder, and \
                this entry is what would have caught it**: it takes both controls to reach \
                that build now, and with both of them `--godray-strength 0` can still change \
                nothing. The row below is the half that stops this being vacuous",
    },
    // ------------------------------------------------------- batch 36's per-block permutation
    Implication {
        base: &[],
        also: &["--no-tex-variation"],
        only: "default,terraces",
        expect: Composes::Something,
        about: "**a `Something` with no `Nothing` beside it, which is the opposite shape to                 every pair above and deliberate.** `--no-tex-variation` clears no other                 control and is cleared by none, so it makes no composition claim for a                 `Nothing` to check. What it does claim is that it is *wired up at all*, and                 that is exactly what six batches of `--water-mottle 0.2 -> Nothing` failed to                 say. The permutation reaches any vantage with ground in it, so a run here                 that moved nothing would mean the flag, the pipeline key or the class table                 had quietly stopped reaching `shade_hit`",
    },
    // ------------------------------------------------- batch 37's second reader of the envelope
    Implication {
        base: &[],
        also: &["--no-distant-shadows"],
        only: "lod,lattice,low-sun,sky",
        expect: Composes::Something,
        about: "**that the ground's reader of the light envelope is wired up at all**, which                 is batch 36's shape and here for batch 35's reason. The four vantages are the                 ones measured to move, and what they have in common is the only condition                 under which a distant terrain shadow can exist: lit far ground under a sun low                 enough that 220 blocks of sun ray has not already climbed over the relief.                 `default`, `terraces`, `overcast`, `underwater`, `cave`, `night` and `edits`                 are 0 pixels against a real revert and none of them is a failure -- naming them                 here is what would make this vacuous",
    },
    Implication {
        base: &["--no-terrain-shafts"],
        also: &["--no-distant-shadows"],
        only: "lod,lattice,low-sun,sky",
        expect: Composes::Something,
        about: "**the claim batch 35's control quietly stopped being able to make.** The                 envelope had one reader and `--no-terrain-shafts` spoke for the whole field; it                 has two now, and this says the haze's reader is not the ground's -- switching                 the shaft off must still leave a distant shadow for the second flag to take.                 It is the same pair, one batch later, that `--cloud-shadow 0` made with                 `--no-terrain-shafts` above",
    },
    Implication {
        base: &["--no-distant-shadows"],
        also: &["--no-terrain-shafts"],
        only: "low-sun,sky",
        expect: Composes::Something,
        about: "the other direction, and the one that says the pair is a pair: with the ground                 term already gone the shaft has to still be there to remove. Without it a                 single flag that had quietly acquired both readers would pass the row above                 and this one would catch it",
    },
    Implication {
        base: &["--cloud-shadow", "0"],
        also: &["--no-terrain-shafts"],
        only: "low-sun,sky",
        expect: Composes::Something,
        about: "**the positive half of the row above, and the reason it is two claims and not \
                one.** A `Nothing` on its own cannot be told from a control that was never \
                wired up -- `--water-mottle 0.2 -> Nothing` passed every run for six batches \
                while being true only of a build whose default was wrong. So this reads the \
                same quantity from the other end: with the deck's shadow already gone, \
                switching the terrain occluder off has to *still* move the frame, which is \
                exactly the claim that batch 35's shaft is not the deck's",
    },
    Implication {
        base: &["--godray-strength", "0"],
        also: &["--cloud-shadow", "0"],
        only: "default,low-sun,sky,coastline",
        expect: Composes::Something,
        about: "**the same two flags, the other way round, and the claim is the opposite one.** \
                `--godray-strength 0` alone keeps the ground shadow, so adding \
                `--cloud-shadow 0` must still move the frame. A directional claim is the \
                easiest kind to have backwards, and the pair is what makes either half \
                falsifiable",
    },
    // -------------------------------------------------------------- batches 19 and 20 compose
    Implication {
        base: &["--no-wave-shoal"],
        also: &["--no-wave-aniso"],
        only: "open-sea,shore,coastline",
        expect: Composes::Something,
        about: "the control table's claim that the two `compose` -- the pair reproducing the \
                pre-batch-19 hash exactly -- is a claim that *neither has absorbed the \
                other's job*. So each must still move the frame in the other's presence",
    },
    Implication {
        base: &["--no-wave-aniso"],
        also: &["--no-wave-shoal"],
        only: "open-sea,shore,coastline",
        expect: Composes::Something,
        about: "and the same in the other order",
    },
    // -------------------------------------------------------------- flags that ship at their default
    Implication {
        base: &[],
        also: &["--water-mottle", "0.2"],
        only: "terraces,lattice",
        expect: Composes::Something,
        about: "**this claim was `Nothing` until batch 26, and the flip is the batch.** 0.2 \
                was the default while three other places in the tree said 0.0 ships, so naming the \
                control really was a no-op -- which is the shape of the defect: an entry here \
                asserting that a control does nothing cannot be told apart from one asserting \
                that a control is not wired up. It moves 16,437 pixels at `terraces` and 82,298 \
                at `lattice` now",
    },
    Implication {
        base: &[],
        also: &["--water-mottle", "0"],
        only: "terraces,lattice",
        expect: Composes::Nothing,
        about: "the other half, and the rendered twin of \
                `the_shipping_mottle_is_the_amplitude_the_tests_call_shipping`: naming the \
                *shipping* amplitude is the no-op. That guard reads `Config::default()` and this \
                one reads the frame, so a batch moving the default without the constant fails \
                one, and a batch moving both fails this",
    },
    Implication {
        base: &[],
        also: &["--cam-submerge", "0"],
        only: "default,coastline",
        expect: Composes::Nothing,
        about: "**0 is the off value, not `put the eye at the surface`.** Batch 21's doc passed \
                it believing otherwise and measured from the default height of 40; batch 22 \
                caught that and dropped it from the `lattice` vantage. This is the check that \
                keeps it caught",
    },
    Implication {
        base: &[],
        also: &["--scale", "1"],
        only: "default,terraces",
        expect: Composes::Nothing,
        about: "batch 22 routed `--scale` through `resize` so that `--screenshot` stops \
                ignoring it. A ratio of 1 has to stay the identity after that -- the one \
                value of the flag whose result is knowable without a reference",
    },
    // ------------------------------------------- batch 41's shadow cut, and its two readers
    Implication {
        base: &[],
        also: &["--no-water-shadow-cut"],
        only: "coastline,open-sea,lattice",
        expect: Composes::Something,
        about: "the positive half, and the only thing a control with no composition to check \
                can claim on its own: that it is wired up at all. `SPEC_WATER_SHADOW_DIST` is \
                a *value* rather than a switch, so unlike every other flag in `SPEC_MASK` the \
                shader never tests this bit -- a typo in `make_spec` would compile, run, and \
                render the shipping frame at every vantage",
    },
    Implication {
        base: &["--no-water-refract"],
        also: &["--no-water-shadow-cut"],
        only: "coastline,open-sea,lattice,terraces",
        expect: Composes::Nothing,
        about: "the refracted ray is the constant's only reader: `shade_water` passes it to \
                the one `shade_hit` call under `FLAG_WATER_REFRACT`, and the reflected leg \
                beside it passes `frame.shadow_dist` like any surface reached through air. So \
                a build that traces no refraction cannot see this control, and a reading of \
                `Something` here would mean the budget had leaked into a second call site",
    },
    // -------------------------------------------------------------- the guard on the guard
    Implication {
        base: &[],
        also: &["--no-water"],
        only: "coastline,terraces",
        expect: Composes::Something,
        about: "the floor of the whole table. If this reads `Nothing`, the fixture is not \
                rendering what it thinks it is rendering and every zero above is worthless",
    },
    // ------------------------------------------ batch 53's dark rung, nested inside batch 48's
    Implication {
        base: &["--no-water-far"],
        also: &["--no-water-dark"],
        only: "deep-water",
        expect: Composes::Nothing,
        about: "the dark rung lives **inside** batch 48's own `if` in `tile_select` and \
                narrows the reach that rung handed it, so a build that never enters the outer \
                block cannot reach this one. `tests/submerged.rs` makes the source half of \
                that claim -- the rung says `min(reach, ...)` and has no `frame.far` to fall \
                back on -- and this is the half a render has to settle. It is also the row \
                that keeps the control table's cost column honest: if this ever read \
                `Something`, `--no-water-far`'s measured saving would silently contain this \
                rung's as well",
    },
    Implication {
        base: &[],
        also: &["--no-water-dark"],
        only: "deep-water",
        expect: Composes::Something,
        about: "the positive half, and this control needs it more than most. **Seventeen of \
                the eighteen vantages are bit-exact under it by construction** -- the rung \
                tests the eye's own depth and no other camera in the set is deep -- so \
                `bitexact` cannot distinguish *this feature is exact* from *this feature never \
                fires*, which is precisely what batch 51 shipped nothing over. Only a reading \
                that says the flag changes something, at the one camera that can trigger it, \
                separates the two",
    },
    // ------------------------------------ batch 54: the surface from below, and what it is not
    Implication {
        base: &[],
        also: &["--no-snell"],
        only: "deep-water",
        expect: Composes::Something,
        about: "the positive half, and this control needs it as badly as batch 53's did. \
                Seventeen of eighteen vantages are bit-exact under it by construction -- the \
                term is unreachable without `FLAG_UNDERWATER`, and the ramp is zero in the top \
                three blocks where the only two submerged vantages sit -- so `bitexact` alone \
                cannot tell *this feature is confined* from *this feature never runs*. It is \
                also the row that would have caught the batch's own shipped bug: the first cut \
                gated the trace and not the term, and `--no-snell` then changed the frame by a \
                max channel delta of 3 instead of 136",
    },
    Implication {
        base: &["--no-water-reflect"],
        also: &["--no-snell"],
        only: "deep-water",
        expect: Composes::Something,
        about: "**two reflections, two controls, and this is the row that keeps them apart.** \
                `--no-water-reflect` is batch 8c's traced reflection of the sky *off* the \
                surface, which only a dry camera can see; this is the mirror a submerged one \
                sees on the underside of the same surface. They share `trace_world` and \
                nothing else, and a reading of `Nothing` here would mean the older flag had \
                quietly swallowed the newer feature -- which is exactly the `FLAG_TERRAIN_SHAFTS` \
                against `FLAG_DISTANT_SHADOWS` hazard, two readers of one field with one control \
                between them",
    },
    // ----------------------------------------- batch 59: the control is exact, not absent
    Implication {
        base: &[],
        also: &["--no-light-rgb"],
        only: "default,cave",
        expect: Composes::Nothing,
        about: "**coloured block light touches block light and nothing else**, and these are \
                the two cameras that can say so from inside one binary. Neither holds an \
                emitter -- no generator places one, and `cave` is an air pocket lit by the \
                ambient floor alone -- so every cell in both frames has all three block \
                channels at zero, both arms of `SPEC_LIGHT_RGB` reduce to `mix(SKY_TINT, .., \
                0.0)`, and the sky half of the read is untouched by the batch. A reading of \
                `Something` here would mean the widened cell had moved the *sky* nibble, which \
                is the one mistake a two-byte repacking is actually prone to",
    },
    Implication {
        base: &[],
        also: &["--no-light-rgb"],
        only: "lamps,edits",
        expect: Composes::Something,
        about: "and the half that keeps the row above honest, which matters more here than \
                anywhere else in this table. `lessons.md` records that *a no-op claim cannot \
                be told apart from a not-wired-up claim*; batch 59's control is bit-exact at \
                **sixteen** of the eighteen vantages against the parent build, and a control \
                that had been dropped by the parser entirely would read exactly the same way. \
                `lamps` is the chamber built for this and moves an MAE of 13.10 over its crop. \
                **`edits` is the more interesting of the two**: it holds glowstone and nothing \
                else, and it moves because that block's emission was re-authored from one \
                scalar level into the three whose `light_curve` values come nearest the \
                retired `BLOCK_TINT` -- 4 bits cannot reach it exactly, so the shipping frame \
                departs from the parent there and the control brings it back",
    },
    // ------------------------------------------- batch 57: the cube is the field, not the code
    Implication {
        base: &["--probe-fill", "1.0"],
        also: &["--no-probe-cube"],
        only: "default,terraces,lod,canopy",
        expect: Composes::Nothing,
        about: "**pinning the field to 1.0 already switches the ambient cube off**, because \
                what `shade_hit` does with the tap is multiply `face_shade` by it. That is the \
                claim the rest of this batch's verification cannot make: `--no-probe-cube` is \
                bit-exact against the parent build at all eighteen vantages, which proves the \
                *control* reverts and says nothing about where the shipping frame's difference \
                comes from. This row says it comes from the contents of the lattice and not \
                from the code path around it -- the same question `--probe-fill` was invented \
                for in batch 55, asked from the other side. **Batch 58 tried to add the twin of\
                this row for the ground bounce and could not**: the same A/B reads 0 pixels at\
                three of these four vantages and **2 at `terraces`, deterministically, at max\
                delta 1**, because the two sides are different *pipelines* and the bounce's\
                multiply lands inside `shade_hit`'s parenthesised sum where this one lands on a\
                scalar factor at the end. So whether a pinned-field A/B is bit-exact is a fact\
                about where the driver folds, not about the design -- see `errors.md`. This row\
                passing is luck that this row happens to have",
    },
    Implication {
        base: &[],
        also: &["--probe-fill", "1.0"],
        only: "default,terraces",
        expect: Composes::Something,
        about: "and the positive half of the row above it, which is the one that would catch \
                the pin going stale. `--probe-fill` has to *reach* the field: if a later edit \
                left the bake writing over a pinned lattice, the row above would still read \
                `Nothing` -- both sides baked -- and batch 55's and batch 56's diagnostics \
                would silently stop measuring the constant field their numbers were read off",
    },
];

// ------------------------------------------------------------------ metric validation

/// How much a metric has to move on its known-positive control before it is allowed to rule
/// anything out.
#[derive(Clone, Copy, Debug)]
pub enum Response {
    /// The metric must differ between the two builds by at least this much.
    DeltaAtLeast(f64),
    /// The control's value must be at most this fraction of the baseline's -- for metrics
    /// where the control *removes* signal rather than changing it.
    RatioAtMost(f64),
    /// The control's value must be at least this fraction of the baseline's.
    ///
    /// **The direction matters and is not decoration.** `coherence`'s control puts the texture
    /// phase *back*, so it must raise the number; and the per-tile readings behind that mean
    /// go both ways, several of them by more than the mean itself. A `DeltaAtLeast` would take
    /// the absolute difference and pass just as happily on a batch that had inverted the
    /// metric's sense.
    RatioAtLeast(f64),
}

/// One metric, one control that is known to move it, and the size of the response.
///
/// The rule this encodes is batch 22's reason for existing, stated as code: **a measure that
/// cannot see a known-positive is not fit to rule anything out.** Batch 21 built three
/// look-metrics and shipped conclusions from two of them before noticing that one could not
/// distinguish frames differing by MAE 18.8.
///
/// The thresholds are floors at roughly half the measured response, not the measured values.
/// A floor catches a metric that has gone blind; an equality would just be a second copy of
/// the number, and would fail on a driver update for no useful reason.
pub struct MetricCheck {
    pub metric: &'static str,
    pub vantage: &'static str,
    /// Empty means the whole frame.
    pub crop: &'static str,
    pub control: &'static [&'static str],
    /// Arguments handed to **both** arms, to take a confound out of a comparison that is
    /// still about `control`.
    ///
    /// **Almost always empty, and batch 39 is why.** That batch was offered exactly this for
    /// `--no-tex-variation` -- `--no-leaf-cutout` on both sides restores its response from
    /// 1.0755x to 1.2053x -- and refused, "because the row would then speak for a build nobody
    /// ships". That refusal stands and this field does not repeal it.
    ///
    /// **What it is for is the case batch 39 did not have: a metric whose *premise* has been
    /// violated, as against one whose *response* has shrunk.** Batch 39's row had honestly got
    /// weaker, because the frame really did hold less in-phase lattice; propping it up would
    /// have certified the instrument against a frame that no longer existed. A confound is the
    /// other thing. `rg_speckle`'s row asserts that absorption is what carries a
    /// high-frequency R-G signal in water; batch 63 gave every face in the world an R-G signal
    /// out of the sky's hue, which is not absorption and was never what the row was pointed
    /// at. Cancelling that on both arms does not change what the row certifies about
    /// absorption -- it is the only way to go on certifying it at all.
    ///
    /// **Two conditions before reaching for this, both of which that row meets and neither of
    /// which is automatic.** The confound has to be *measured* rather than assumed: batch 65
    /// put a Fresnel reflection of the sky on every opaque surface in the world and moved that
    /// row by **0.008**, so the list is one flag and not one per appearance batch, which is
    /// what a prop that had to grow would look like. And **its companion row must stay
    /// unpropped**, so that the pair still has one member speaking for the build that ships --
    /// `rg_std` measures the same control on the same crop with this empty and passes on the
    /// shipping frame at 0.3427x. A pair with both halves isolated would be batch 39's
    /// objection with extra steps.
    pub both: &'static [&'static str],
    pub expect: Response,
    pub about: &'static str,
}

pub const CHECKS: &[MetricCheck] = &[
    MetricCheck {
        metric: "mae",
        vantage: "lamps",
        crop: "mixing-wall",
        control: &["--no-light-rgb"],
        both: &[],
        expect: Response::DeltaAtLeast(6.0),
        about: "**the only row in this table whose vantage cannot be a `bitexact` row**, and \
                that is why it exists. `--demo-lamps` builds its chamber out of block ids that \
                did not exist before batch 59, and `block::def` clamps an unknown id to the \
                last row of a shorter table -- so the journal substitution `harness.md` records \
                for `--demo-glass` would replay this room as *meadow* rather than refusing it, \
                which is the one failure mode indistinguishable from success. The eighteen \
                cameras that predate the batch prove the control **reverts**; nothing there can \
                prove the feature **arrives**, because no generator places a lamp and glowstone \
                is the only emitter any of them holds. This is that half. Measured **13.10** \
                over the crop, every one of its 79,772 pixels differing at max delta 45; the \
                floor is a little under half, which is where a chamber whose lamps had drifted \
                to two blocks further out would still pass",
    },
    MetricCheck {
        metric: "mae",
        vantage: "terraces",
        crop: "water",
        control: &["--no-water-refract"],
        both: &[],
        expect: Response::DeltaAtLeast(18.0),
        about: "dropping the refracted ray moves every water pixel: 37.96 measured over the \
                terraces water crop. **This check was on `lattice` until batch 21b and had to \
                move**, which is a finding rather than a maintenance chore. There the same \
                control was worth 15.77 before the batch and 3.99 after it, because \
                `--no-water-refract` leaves `refr = body` and `body` used to be the *glass* \
                layer's pale near-white -- nothing like the water it stood in for, so removing \
                the refraction was a huge change. With the right layer `body` really is what \
                the refracted image converges to, so dropping the refraction in deep water is \
                a small change and in the shallow terraces water still a large one",
    },
    MetricCheck {
        metric: "pixels",
        vantage: "lattice",
        crop: "",
        control: &["--no-water-refract"],
        both: &[],
        expect: Response::DeltaAtLeast(200_000.0),
        about: "the same control counted in pixels rather than in code values -- 370,964 of \
                921,600 measured, and unaffected by batch 21b because which pixels *move* was \
                never the thing the glass layer distorted",
    },
    MetricCheck {
        metric: "max",
        vantage: "coastline",
        crop: "water",
        control: &["--no-waves"],
        both: &[],
        expect: Response::DeltaAtLeast(20.0),
        about: "a flat sea against a wavy one, inside the water crop: 93 code values measured",
    },
    MetricCheck {
        metric: "speckle",
        vantage: "open-sea",
        crop: "water",
        control: &["--no-waves"],
        both: &[],
        expect: Response::RatioAtMost(0.75),
        about: "0.660 to 0.345 measured, a 48% response. **Batch 22 tried exactly this control \
                here, measured 0.909x, called it weak and used `--mip-bias 3` instead; batch \
                21b inverts both halves of that.** On the pre-21b binary this control really \
                does read 0.787 to 0.715, and batch 22 explained the 9% as batch 19's band \
                limit having already faded the short octaves out -- plausible, quoted forward, \
                and wrong. The wave field was a small share of the residual because \
                `shade_water` was sampling the *glass* layer, whose pane borders and \
                `(x + y) % 9` streaks were most of the detail on distant water. With the right \
                layer the sea's own texture is nearly smooth, so the waves are what is left: \
                `--mip-bias 3` has gone the other way, from 0.37x to **0.96x**, and is no \
                longer fit to be anything's known-positive",
    },
    MetricCheck {
        metric: "coherence",
        vantage: "default",
        crop: "slope",
        control: &["--no-tex-variation"],
        both: &[],
        expect: Response::RatioAtLeast(1.037),
        about: "**the known-positive is the defect the metric was built for**, and it is now \
                the weakest row in this table by a factor of five. Re-fitted in batch 39 \
                from 1.10, against 0.1420 to 0.1527 measured \
                -- a 7.55% response, against the **20.53%** batch 36b fitted the old 1.10 floor \
                to. **What fell is the frame and not the instrument, and the tile readings say \
                so exactly**: of batch 36b's 0.0309 response, 35 of the 190 tiles carried **79% \
                of it**, and those 35 are the tiles holding tree canopy. Batch 38 carved the \
                canopy into a 4^3 mask, those tiles went from +0.1330 to +0.0266, and every \
                canopy-free tile is unchanged to four decimals. So the in-phase lattice this \
                control was mostly moving was never `GRASS_SIDE`'s corduroy on the slope the \
                crop is named for -- it was the **`LEAVES` layer, D4-permuted, on solid leaf \
                cubes**, and batch 38 was accidentally the larger share of a texture-repetition \
                fix. Re-fitted rather than re-pointed, because nothing else in the fixture is \
                stronger: this control is worth 1.0736x at `coastline/water` and under 1.02x \
                everywhere else, and it runs the *wrong* way at `shore`, `terraces` and \
                `underwater`. Re-fitted rather than propped up with `--no-leaf-cutout` on both \
                sides, which restores 1.2053x and was declined because the row would then speak \
                for a build nobody ships. All four constants were re-swept against the carved \
                frame and none recovers it -- the best is 1.0853x at `lag_max` 24, a third \
                decimal. **The floor is half the response, and at 1.037 this row can now only \
                catch total blindness**; the claim that the instrument is not blind belongs to \
                the synthetic lattice pair in `tests/harness.rs`, which is content-independent \
                and reads 12.7x. It is a `RatioAtLeast` because putting the phase back has to \
                make the frame *more* regular, not merely different",
    },
    MetricCheck {
        metric: "rg_std",
        vantage: "terraces",
        crop: "water",
        control: &["--water-absorb", "0"],
        both: &[],
        expect: Response::RatioAtMost(0.35),
        about: "batch 21b's diagnostic metric against the term that drives it -- clear water \
                cannot carry a path-length or a per-block colour signal at all. 5.713 to 1.012 \
                measured; it read 6.474 before the glass layer was taken out of `body`",
    },
    MetricCheck {
        metric: "rg_speckle",
        vantage: "terraces",
        crop: "water",
        control: &["--water-absorb", "0"],
        // **The one row in this table with a both-sides argument, added by roadmap D4.** See
        // `MetricCheck::both` for the rule, and for why batch 39's refusal to prop a row is
        // not repealed by it. The short version is that this row's *premise* was violated
        // where batch 39's *response* had merely shrunk.
        both: &["--no-sky-tint"],
        expect: Response::RatioAtMost(0.8),
        about: "the *high-frequency* half of the same signal, and the metric batch 21b had to \
                add because `rg_std` cannot be an acceptance test: it charges the smooth depth \
                cue and the per-voxel lattice together, so a change that deleted both would \
                score as a triumph. 0.277 to 0.174 measured. The pair is what says the batch \
                removed a defect rather than a feature -- the glass layer cost **0.823** here \
                against this same 0.174 floor, so the fix took 85% of the excess out while \
                `rg_std` stayed at 5.7 of a possible 6.5. \n                **`--no-sky-tint` is on both arms since roadmap D4, and the row failed for \n                two batches before it was.** Batch 63 made the ambient's sky colour a derived \n                hue *per face*, so a submerged surface under clear water began carrying an \n                R-G signal that has nothing to do with absorption, and this ratio -- which \n                has to *fall* -- rose instead: 0.4993x to 1.2722x on the binary built from \n                batch 63's own commit. The correction is to the check and not to the shader. \n                **It is one flag and not a growing list, which is the measurement that made \n                it isolation rather than a prop**: batch 65 put a Fresnel reflection of the \n                sky on every opaque surface in the world and moved this row by **0.008**, \n                0.5156x to 0.5073x. **`rg_std` above is deliberately left unpropped** and \n                passes on the shipping frame at 0.3427x, so batch 21b's pair still has one \n                member speaking for the build that ships",
    },
    MetricCheck {
        metric: "pixels",
        vantage: "sea-horizon",
        crop: "",
        control: &["--no-water-far"],
        both: &[],
        expect: Response::DeltaAtLeast(150.0),
        about: "**the row this table was missing when batch 48 needed it**, and the only one \
                here whose job is to police a *vantage* rather than a metric. Batch 48 clamped \
                how far a submerged camera tests chunks, swept the reach from 32 blocks \
                upward, and got zero pixels at every value and at every extinction down to \
                `--water-absorb 0.05` -- then found that `underwater`'s submerged content ends \
                within 32 blocks, so the sweep had been measuring the camera. The zeros were \
                not margin. Every other row here answers *can this metric see a change*; this \
                one answers *can this fixture see this feature at all*, which is the question \
                a clean sweep is least able to raise about itself. **The floor is fitted to \
                the weakest reach anyone would defend and not to the shipped one**, which is \
                the opposite of how every other row here is set and is deliberate. At the \
                shipped 128 this control moves **67,621 pixels**; at 256, which batch 48 \
                shipped and batch 52 measured to be 8 blocks under exact, it moves **316**. \
                Half of 67,621 would make this row an assertion about `WATER_FAR_DIST`, and it \
                would fail the day somebody legitimately put the reach back up -- so the floor \
                is half of **316**, and what it polices is the camera. **It is also the whole \
                frame and not the `horizon` crop**: the claim is that the reach reaches the \
                image somewhere, and narrowing it to where batch 52 happened to find the \
                content would make the row agree with that batch by construction",
    },
    MetricCheck {
        metric: "pixels",
        vantage: "deep-water",
        crop: "",
        control: &["--no-water-dark"],
        both: &[],
        expect: Response::DeltaAtLeast(900.0),
        about: "the row above one batch on, policing the same thing in a sharper way. Batch \
                48's rung could be *seen* at any submerged camera once one existed; batch \
                53's can only be **triggered** by a camera whose own eye is under the sky \
                flood's reach, which is `light::MAX_LEVEL` = 15 blocks down. Every other \
                vantage in the set -- `underwater` and `sea-horizon` included, both three \
                blocks down -- is bit-exact under this control **by construction**, so a \
                sweep of the other seventeen is not weak evidence, it is no evidence, and it \
                reads identical to a pass. `deep-water` sits 24 blocks down and is the whole \
                of the fixture's ability to see this feature. **The floor is fitted to the \
                retreat rung rather than to the shipped one**, the way the row above is: the \
                shipped 64 moves **19,576 pixels at max delta 3**, and 96 -- the only value \
                above it that does anything, since the cull is per chunk AABB and 48, 32, 24 \
                and 16 all render the identical frame -- moves **1,993 at max delta 2**. Half \
                of 1,993 is what a legitimate retreat has to clear",
    },
    MetricCheck {
        metric: "pixels",
        vantage: "deep-water",
        crop: "",
        control: &["--no-snell"],
        both: &[],
        expect: Response::DeltaAtLeast(100000.0),
        about: "the third row in a row whose job is to police a *camera*, and the strictest of \
                them. Batch 53's dark rung needed a submerged eye; this one needs a submerged \
                eye **more than `SNELL_MIN_DEPTH` blocks down**, because the user's own \
                correction to the first build was that the top three blocks of water must not \
                mirror at all. So `underwater` and `sea-horizon` -- the only other submerged \
                vantages, both exactly three blocks down -- are bit-exact under this control \
                **by construction and on purpose**, and seventeen clean rows say nothing \
                whatever about whether this feature works. `deep-water` at 24 blocks is the \
                entire ability of this fixture to see it. **The floor is a quarter of the \
                measured 415,642 rather than half**, and the departure from the two rows above \
                is deliberate: those police a constant that could legitimately be retreated, \
                where what can legitimately move here is the *vantage's depth*, and the \
                response falls off with it along the ramp rather than in steps. A quarter is \
                where a camera that had drifted to about seven blocks would fail",
    },
    MetricCheck {
        metric: "mae",
        vantage: "offscreen-shadow",
        crop: "shadowed-slope",
        control: &["--no-offscreen-shadows"],
        both: &[],
        expect: Response::DeltaAtLeast(1.3),
        about: "the fourth row in a row policing a *camera* rather than a metric, and the one \
                whose camera had to be built from an angle rather than from a depth. Roadmap \
                D3 is terrain outside the view frustum casting no shadow; for that to reach a \
                frame at all the occluder has to be **more than 52 degrees off the view axis** \
                -- the frustum's own horizontal half-angle, below which nothing is culled -- \
                and **within `shadow_dist`'s 220 blocks** of the ground it darkens, past which \
                batch 37's light envelope carries the shadow off a height window no frustum \
                touches. **Sixteen of the nineteen cameras that predate batch 64 satisfy \
                neither condition and are bit-exact under this control**, so a clean sweep of \
                them is no evidence and reads exactly like a pass. **The three that do move are \
                why this row is an MAE and not a count**, and it is where the three rows above \
                do not transfer: unlike their vantages this one is *not* the whole of the \
                fixture's ability to see the feature -- `terraces` moves 7,843 pixels and \
                `lattice` 32 -- so a count here would be satisfied by a camera that had drifted \
                onto somebody else's response. What this vantage is alone in is **rank**: \
                2.6165 over its own crop against a whole-frame 0.2552. **That is batch 63's \
                lesson applied on purpose**, because the same control at `--time \
                0.28` moves nearly twice as many pixels for a fifth of the MAE, so a count \
                would rank the two cameras backwards and would pass on a build whose shadow \
                had gone faint. Measured **2.6165** at max delta 27 over 15,159 of the crop's \
                pixels; the floor is half",
    },
];



