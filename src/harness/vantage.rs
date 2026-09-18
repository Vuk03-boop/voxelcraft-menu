
//! The named vantage set, and the crops scored at each one.
//!
//! Before batch 22 these lived as prose in `PERF.md` and in fourteen batch documents, and got
//! retyped -- with drift -- every session. Three consequences, all of them real and all of them
//! paid for at least once:
//!
//! - Batch 20 quotes a "shore" vantage in four tables and records its arguments in exactly one
//!   place, in `PERF.md`, with the time of day left off.
//! - Batch 21 describes its measurement vantage as "eye at the waterline" and passes
//!   `--cam-submerge 0`, which is the *off* value -- the capture was taken from the default
//!   height of 40. The numbers are reproducible from the arguments; the sentence is not.
//! - Crops were picked per session, and one of them quietly contained a shoreline edge, which
//!   is how an FFT band peak came to read near-constant across builds differing by MAE 18.8.
//!
//! So the set is data. A vantage is a name and the exact argument list; a crop is a name and a
//! rectangle in **fractions of the frame**, so it means the same region at any resolution.
//!
//! `tests/harness.rs` runs every argument list here through the real command-line parser and
//! fails on anything it does not recognise, which is what stops this table from outliving a
//! flag.

use super::metric::Rect;

/// What a crop is supposed to contain. The point of the tag is that it is *checkable*:
/// `harness validate` renders the vantage with and without water and confirms that a
/// [`Content::Water`] crop moves everywhere and a [`Content::Land`] crop does not move at all.
/// A crop that has drifted onto a shoreline fails both tests and says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// Every pixel is water. Must move under `--no-water`.
    Water,
    /// No pixel is water, and none is sky. Must not move under `--no-water`.
    Land,
    /// Sky and cloud only.
    Sky,
    /// Deliberately mixed; no automatic check applies.
    Mixed,
}

impl Content {
    pub fn label(&self) -> &'static str {
        match self {
            Content::Water => "water",
            Content::Land => "land",
            Content::Sky => "sky",
            Content::Mixed => "mixed",
        }
    }
}

/// A rectangle in fractions of the frame, `x1`/`y1` exclusive.
#[derive(Clone, Copy, Debug)]
pub struct Crop {
    pub name: &'static str,
    pub content: Content,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Crop {
    /// Resolve to pixels. Rounding rather than truncation, so the same crop at 1280x720 and
    /// at 960x540 covers the same fraction of the frame to within half a pixel.
    pub fn rect(&self, w: u32, h: u32) -> Rect {
        Rect {
            x0: (self.x0 * w as f32).round() as u32,
            y0: (self.y0 * h as f32).round() as u32,
            x1: (self.x1 * w as f32).round() as u32,
            y1: (self.y1 * h as f32).round() as u32,
        }
    }
}

pub struct Vantage {
    pub name: &'static str,
    /// Exactly the arguments to pass, minus the capture recipe (`--screenshot`, size, TAA).
    pub args: &'static [&'static str],
    pub about: &'static str,
    pub crops: &'static [Crop],
}

impl Vantage {
    pub fn crop(&self, name: &str) -> Option<&Crop> {
        self.crops.iter().find(|c| c.name == name)
    }
}

/// The capture recipe every number in `docs/harness.md` was taken with.
///
/// 1280x720 because that is what batches 11, 14, 15, 19, 20 and 21 used; `--no-taa` because a
/// temporal pass hides exactly the high-frequency residual the look-metrics exist to measure,
/// and because a single frame is the deterministic thing. The bit-exactness sweep runs the TAA
/// variant as well -- `--taa` on `harness capture` and `harness bitexact` -- since a control
/// that is bit-exact without the temporal pass and not with it would be a real finding.
pub const STD_WIDTH: u32 = 1280;
pub const STD_HEIGHT: u32 = 720;

/// The vantage set. The first nine are the set batch 11 established and every batch since
/// has drawn from; `shore`, `open-sea`, `lattice` and `terraces` are the water vantages
/// batches 19 to 21 added, and `edits` is the one path that exercises a player edit.
///
/// The rest were each added for one thing the set could not rank: `glass` (38b), `canopy`
/// (43), `sea-horizon` (52), `deep-water` (53) and `lamps` (59). **The count this sentence
/// used to carry has been wrong twice** -- it read "fourteen" through batches 38b and 43,
/// which added the fifteenth and sixteenth, and "seventeen" through batch 59 -- so it does not
/// carry one any more. Count the entries, the way `ledger.md` says to count its rows.
pub const VANTAGES: &[Vantage] = &[
    Vantage {
        name: "default",
        args: &["--time", "0.30"],
        about: "the default camera: a grassy mountain filling the frame, water at the right edge",
        crops: &[Crop {
            name: "slope",
            content: Content::Land,
            x0: 0.05,
            y0: 0.45,
            x1: 0.55,
            y1: 0.95,
        }],
    },
    Vantage {
        name: "coastline",
        args: &[
            "--time",
            "0.74",
            "--cam-height",
            "24",
            "--cam-pitch",
            "-6",
            "--cam-yaw",
            "143",
        ],
        about: "down a coastline into a low sun -- the expensive vantage for anything a \
                secondary ray reads, since `shade_water` calls `sky_color` unconditionally",
        crops: &[
            Crop {
                name: "water",
                content: Content::Water,
                x0: 0.42,
                y0: 0.64,
                x1: 0.78,
                y1: 0.95,
            },
            Crop {
                name: "sky",
                content: Content::Sky,
                x0: 0.05,
                y0: 0.02,
                x1: 0.35,
                y1: 0.16,
            },
        ],
    },
    Vantage {
        name: "open-sea",
        args: &[
            "--time",
            "0.74",
            "--cam-submerge",
            "-60",
            "--cam-pitch",
            "-8",
            "--cam-yaw",
            "143",
        ],
        about: "sixty blocks above open water: batch 19's vantage, where the grazing footprint \
                correction is worth 15.9x at the horizon",
        crops: &[Crop {
            name: "water",
            content: Content::Water,
            x0: 0.32,
            y0: 0.64,
            x1: 0.68,
            y1: 0.95,
        }],
    },
    Vantage {
        name: "shore",
        args: &[
            "--cam-submerge",
            "-1",
            "--cam-yaw",
            "-219",
            "--cam-pitch",
            "-4",
        ],
        about: "one block above the sea looking along it -- nothing but shallow water, which is \
                the only water batch 20 does any work for",
        crops: &[Crop {
            name: "water",
            content: Content::Water,
            x0: 0.38,
            y0: 0.62,
            x1: 0.95,
            y1: 0.97,
        }],
    },
    Vantage {
        name: "lattice",
        args: &["--cam-pitch", "-10", "--cam-yaw", "-122"],
        about: "batch 21's measurement vantage, over a wide shallow lagoon. Its doc calls this \
                \"eye at the waterline\" and passes `--cam-submerge 0`, which is the off value: \
                the eye is at the default height of 40. The arguments here are the ones that \
                were run, with the no-op dropped",
        crops: &[Crop {
            name: "water",
            content: Content::Water,
            x0: 0.22,
            y0: 0.60,
            x1: 0.68,
            y1: 0.90,
        }],
    },
    Vantage {
        name: "terraces",
        args: &["--cam-submerge", "-2"],
        about: "two blocks above the sea, pitched down over a terraced bottom: batch 21b's \
                vantage, where the refraction's per-voxel path quantisation shows as salmon \
                diamonds on the submerged floor and the dry terraces beside them stay clean",
        // The waterline at this vantage is a clean diagonal from about (0.88, 0.38) to
        // (0.00, 0.90), and the sand *below* it is submerged sand seen through water rather
        // than dry beach -- which is exactly the kind of thing a crop picked by eye gets
        // wrong. The first `dry` crop here was placed on those terraces and came back 38%
        // water; `harness validate` is what said so, on the pixel, before any number was
        // quoted from it.
        crops: &[
            Crop {
                name: "water",
                content: Content::Water,
                x0: 0.55,
                y0: 0.78,
                x1: 0.98,
                y1: 0.98,
            },
            Crop {
                name: "dry",
                content: Content::Land,
                x0: 0.05,
                y0: 0.10,
                x1: 0.55,
                y1: 0.50,
            },
        ],
    },
    Vantage {
        name: "lod",
        args: &[
            "--time",
            "0.26",
            "--cam-yaw",
            "37",
            "--cam-pitch",
            "-20",
            "--cam-height",
            "240",
        ],
        about: "240 blocks up: the only vantage that puts a meaningful share of the frame in an \
                LOD transition band. At eye level a fade A/B moves 0.05% of pixels; here it \
                moves 13%",
        crops: &[],
    },
    Vantage {
        name: "low-sun",
        args: &["--time", "0.26", "--cam-yaw", "37", "--cam-pitch", "0"],
        about: "into the sun at the horizon. `sun()` reaches the horizon at 0.26, not at the \
                0.05 an early roadmap claimed -- 0.05 is the middle of the night",
        crops: &[],
    },
    Vantage {
        name: "sky",
        args: &[
            "--time",
            "0.26",
            "--cam-yaw",
            "37",
            "--cam-pitch",
            "20",
            "--cam-height",
            "60",
        ],
        about: "pitched up into a sky-filling frame",
        crops: &[Crop {
            name: "sky",
            content: Content::Sky,
            x0: 0.20,
            y0: 0.05,
            x1: 0.80,
            y1: 0.35,
        }],
    },
    Vantage {
        name: "overcast",
        args: &["--time", "0.50", "--cloud-cover", "0.70"],
        about: "noon under heavy cover",
        crops: &[],
    },
    Vantage {
        name: "underwater",
        args: &["--time", "0.26", "--cam-submerge", "3"],
        about: "the eye three blocks under the surface: the only way to reach the underwater \
                medium headlessly",
        crops: &[],
    },
    Vantage {
        name: "sea-horizon",
        // **The seventeenth, added by batch 52, and it exists to rank one constant.**
        // `underwater` is a submerged eye with nothing in front of it: its content ends
        // within 32 blocks, so every `WATER_FAR_DIST` above 32 is bit-exact there and batch
        // 48 shipped 256 on the medium's extinction rather than on a capture. Roadmap P4
        // named the missing camera exactly -- *a submerged eye along the horizontal across
        // open water with lit geometry 150 to 400 blocks out* -- and this is it.
        //
        // **`--cam-pitch 0` is the load-bearing argument and it is not a no-op**: the default
        // pitch is -14, and the clamp only fires for a tile whose most *upward* corner ray is
        // still under water at the reach. Three blocks down that is `atan(3/256)` = 0.67
        // degrees, so the clamped region is everything at or below the horizon and nothing
        // above it -- which is why the frame has to be *aimed* at the horizon rather than
        // down at the floor. The measured split says so exactly: above y = 0.43 the reach
        // moves **zero** pixels at any value, because those tiles never clamp.
        //
        // Yaw 215 at the default seed puts a chain of wooded islands along the horizon out to
        // 264 blocks with open water in front of them and a submerged shelf in the near right
        // corner. `--time 0.35` for `canopy`'s reason -- full daylight, so the distant shelf
        // is lit and the crop is not flattering the constant by hiding what it deletes.
        //
        // **Every argument predates the batch**, so this is a `bitexact --exe-b` row from the
        // day it was added.
        args: &[
            "--time",
            "0.35",
            "--cam-submerge",
            "3",
            "--cam-yaw",
            "215",
            "--cam-pitch",
            "0",
        ],
        about: "a submerged eye looking along the horizontal across open water at a chain of \
                wooded islands: the only vantage where the reach a submerged camera stops \
                testing chunks at decides anything, and the only one that is not bit-exact \
                under `--no-water-far`",
        // **The pair is the finding, not either crop on its own.** The shipped 128-block reach
        // moves 67,621 pixels of 921,600 against an *unlimited* one -- and 62,589 of them are
        // in `deep`, at a max delta of **2**, while 5,032 are in `horizon` at a max delta of
        // **23**. A batch reading only the count would rank this truncation by the wrong band,
        // which is `lessons.md`'s *rank a truncation by the content of the band it removes*,
        // wired into the fixture instead of written down again.
        //
        // (Against the *previous* 256 the count is 67,305 instead, and the 316 difference is
        // what 256 was itself throwing away. `bitexact` reads that one; every look claim
        // reads the unlimited-referenced 67,621.)
        crops: &[
            Crop {
                // **`Mixed`, and here that is a statement about the tags rather than about
                // the crop.** Both of these read **100.00%** moved under `validate`'s
                // water-off set, so both would pass a `Water` tag -- and the tag would be
                // vacuous, because at a submerged vantage that control changes the *medium
                // in front of* every pixel rather than the pixels themselves. A crop of bare
                // rock would score 100% here too. `Water` and `Land` mean something at
                // `terraces`, where the control separates sea from beach; under water there
                // is nothing for them to separate, which is why `underwater` carries no crop
                // at all. A tag that cannot fail is worse than no tag, because it reads like
                // a check. What these crops are *for* is `WATER_FAR_DIST`, and `CHECKS` is
                // where a claim about that lives.
                name: "horizon",
                content: Content::Mixed,
                x0: 0.00,
                y0: 0.43,
                x1: 1.00,
                y1: 0.52,
            },
            Crop {
                // Open water below the horizon, out to the near shelf. This is where the
                // *count* lives and the look does not.
                name: "deep",
                content: Content::Mixed,
                x0: 0.00,
                y0: 0.54,
                x1: 1.00,
                y1: 0.74,
            },
        ],
    },
    Vantage {
        name: "deep-water",
        // **The eighteenth, added by batch 53, and it exists for the same reason the
        // seventeenth did one batch earlier: a constant nothing in the set could see.**
        // Roadmap P4's depth sub-lead is the user's own report -- *"the only time it looks odd
        // from inside is when I go too deep"* -- and **no vantage in the fixture was deep**.
        // `underwater` and `sea-horizon` both sit three blocks down, which is less than a
        // quarter of the depth at which the sky flood runs out, so the rung this vantage
        // ranks cannot fire at either of them and both would have agreed with anything.
        //
        // **24 blocks down is the deepest `find_water` reaches near the spawn column**, and
        // the search wanders to do it: 30 finds nothing at all. What 24 buys is the only
        // number that matters here -- it is past `WATER_DARK_DEPTH`'s 15, so a ray that never
        // rises is unlit for its whole length, and the rung fires on the camera rather than
        // on the pitch.
        //
        // **`--cam-pitch 0` again, and for `sea-horizon`'s reason turned inside out.** There
        // the clamped region was everything at or below the horizon; here the camera is deep
        // enough that *below* the horizon is all near sea floor within a few dozen blocks,
        // and the content a reach can delete is in a narrow band of near-horizontal rays just
        // **above** it -- the ones that graze forward through deep water for hundreds of
        // blocks while rising a few. Measured before the batch was written: against an
        // unlimited reach the shipped 128 moves 36,820 pixels here, **every one of them
        // between y = 0.33 and y = 0.54** and none at all below. A vantage pitched down would
        // have had the near floor in front of every ray and nothing to cut.
        //
        // Yaw 215 for `sea-horizon`'s reason as well -- open water rather than the channel
        // wall the search sits beside at yaw 0 and 90 -- and `--time 0.35` so the far sea
        // floor is as lit as this depth ever lets it be.
        args: &[
            "--time",
            "0.35",
            "--cam-submerge",
            "24",
            "--cam-yaw",
            "215",
            "--cam-pitch",
            "0",
        ],
        about: "an eye 24 blocks down over open sea floor, looking along the horizontal: the                 only vantage deep enough for the sky flood to have run out, and so the only                 one where the dark-water rung of the submerged reach decides anything",
        crops: &[
            Crop {
                // `Mixed` for `sea-horizon`'s reason, which applies here with nothing changed:
                // under water the water-off control set changes the medium in front of every
                // pixel, so a `Water` tag would read 100% off bare rock and a `Land` tag could
                // not pass anywhere. A tag that cannot fail is worse than no tag.
                //
                // **This is where the whole of the reach lives at this camera.** The band of
                // near-horizontal rays above the horizon: 34,909 of the 36,820 pixels the
                // shipped 128 already moves are inside it.
                name: "graze",
                content: Content::Mixed,
                x0: 0.00,
                y0: 0.34,
                x1: 1.00,
                y1: 0.54,
            },
            Crop {
                // The near sea floor, which **no** reach reaches: it is within a few dozen
                // blocks of the eye in every downward direction, so every rung above 32
                // leaves it untouched. It is here as the partner that must not move, which is
                // the half `sea-horizon` does not carry -- both of its crops respond.
                name: "floor",
                content: Content::Mixed,
                x0: 0.00,
                y0: 0.62,
                x1: 1.00,
                y1: 0.96,
            },
        ],
    },
    Vantage {
        name: "cave",
        args: &["--time", "0.30", "--cam-height", "-20"],
        about: "inside a real air pocket, found by search rather than by hoping",
        crops: &[],
    },
    Vantage {
        name: "night",
        args: &["--time", "0.05"],
        about: "the middle of the night",
        crops: &[],
    },
    Vantage {
        name: "edits",
        // The height and pitch are not decoration. `--demo-edits` places its hut 14 blocks
        // **horizontally** in front of the camera and then drops it to the surface, so from
        // the default camera -- 40 blocks up, pitched 14 degrees down -- it lands about 40
        // blocks below a frame whose lower edge reaches 16 blocks down at that range, and the
        // capture comes back **bit-identical to the plain default vantage**. This was the
        // fixture's first bit-exactness sweep finding, and it is retrospective: batch 12 lists
        // `--demo-edits` as one of the nine vantages it verified `--no-tint` against, and at
        // that camera the vantage could only ever have been a second copy of the default one.
        args: &["--demo-edits", "--cam-height", "14", "--cam-pitch", "-20"],
        about: "a lit structure built through the normal edit path, which is the only headless \
                capture that exercises one -- from a camera low enough to see it, which the \
                default is not",
        crops: &[],
    },
    Vantage {
        name: "glass",
        // **The fifteenth, added by batch 38b, and the same camera as `edits` on purpose.**
        // `--demo-glass` puts its glasshouse exactly where `--demo-edits` puts its hut, so the
        // two vantages differ in the structure and in nothing else and the pair reads as one
        // A/B. It is a separate vantage rather than glass added to that hut because `edits`
        // has been a reference since batch 29 and half a dozen batches quote numbers off it.
        //
        // It is also the *only* vantage in this set that holds any glass, which is what makes
        // batch 38b's claim a pair rather than a bare no-op: the fourteen above are bit-exact
        // against the pre-38b binary by construction -- no generator places glass -- and this
        // one has to move, or the feature was never wired up.
        args: &["--demo-glass", "--cam-height", "14", "--cam-pitch", "-20"],
        about: "a glasshouse built through the normal edit path: the only capture holding a \
                transmissive block, and the only one whose interior is lit through a closed \
                roof rather than through a hole in it",
        crops: &[Crop {
            name: "pane",
            // `Land` and not `Mixed`, and the tag earns its keep here: the check is that the
            // crop does not move under the water controls, which is a real claim about a
            // structure dropped at whatever surface the camera happens to face. A glasshouse
            // that ever landed on a shoreline would fail it rather than quietly scoring a
            // water crop as a glass one. The sky in the roof's reflection is radiance and not
            // content -- no pixel here is a sky *hit*.
            content: Content::Land,
            x0: 0.33,
            y0: 0.40,
            x1: 0.70,
            y1: 0.89,
        }],
    },
    Vantage {
        name: "glass-sea",
        // **The twenty-first, added by batch 67, and it exists because the twenty before it
        // were blind to what that batch changed.** `shade_glass` dispatches a water hit to a
        // different shading path than a land hit, and until batch 67 that path was a second
        // complete inline of `shade_water` -- the last thing holding `resolve` at 96 registers,
        // worth 0.103 ms at `cave` and 0.399 at `coastline`, neither of which contains a pane.
        // Replacing it with an analytic water colour moved **0 pixels at all twenty vantages**,
        // including `glass`, whose own crop comment says a glasshouse "that ever landed on a
        // shoreline would fail" its `Land` tag -- so the fixture had deliberately excluded the
        // only configuration that could see this.
        //
        // **That is batch 52's lesson arriving a second time: a blind vantage is not a
        // conservative one.** Twenty rows of zero could not argue that the change was safe, only
        // that nobody was looking. Here the change measures **MAE 0.5990 at max delta 37 over
        // 123,079 pixels**, which is under [ADR 0001]'s 2.87 rejected-by-eye line and is a
        // ranking rather than an absence of one.
        //
        // `--cam-submerge 1` is what makes it work: `build_glass_structure` drops the house at
        // whatever surface the camera faces, so an eye one block above the sea puts panes over
        // water. Three nearby cameras were tried and read 0 -- at submerge -1, 0 and -2 the
        // house lands on the beach -- which is why the number here is 1 and not a round figure.
        args: &[
            "--demo-glass",
            "--cam-submerge",
            "1",
            "--cam-yaw",
            "143",
            "--cam-pitch",
            "-25",
        ],
        about: "a glasshouse standing in the shallows: the only capture in which a pane has                 water behind it, and therefore the only one that can see how water seen                 *through* glass is shaded",
        crops: &[Crop {
            // `Mixed` rather than `Water`: the pane's own reflection is sky radiance and the
            // frame behind it is sea, so neither single tag is honest about this rectangle.
            name: "wet-pane",
            content: Content::Mixed,
            x0: 0.30,
            y0: 0.45,
            x1: 0.72,
            y1: 0.92,
        }],
    },
    Vantage {
        name: "lamps",
        // **Added by batch 59, and the only vantage in this set with an emitter in it that is
        // not white.** The three above it that hold any block light at all -- `edits`,
        // `glass`, and `cave` which holds none -- carry glowstone and nothing else, so before
        // this camera existed a coloured flood could have shipped bit-exact everywhere and the
        // sweep would have called it a pass. That is batch 55's own warning about roadmap R4,
        // which is why the entry says the content *is* the batch.
        //
        // **`--cam-height -20` is `cave`'s camera and that is deliberate**: the chamber is
        // built around wherever `find_cave` puts the eye, so the two vantages stand in the same
        // place and differ in whether there is a lit room there. `--time 0.30` is set for the
        // same reason it is set at `cave` -- the room is sealed, so daylight reaches no pixel
        // in it, and a fixed time keeps the *outside* of the world identical to `cave`'s.
        //
        // **It cannot be a `bitexact` row against a parent binary and never will be.** The
        // structure holds block ids that did not exist before the batch, and `block::def`
        // clamps an unknown id to the last row of a shorter table -- so the journal trick
        // `docs/harness.md` records for `--demo-glass` replays this chamber as meadow rather
        // than refusing. Its positive claim comes from `--no-light-rgb` in one binary.
        // **`--cam-yaw 0 --cam-pitch 0` and both are load-bearing.** The room is
        // axis-aligned and `build_lamp_chamber` puts the lamps in whichever wall the look
        // direction points at, so an off-axis yaw spreads the row across a corner and the
        // seams land on two walls at two angles. Square on, the four pools are one band
        // across one plane and the three seams between them are the only thing in the
        // middle of the frame.
        args: &["--demo-lamps", "--time", "0.30", "--cam-height", "-20", "--cam-yaw", "0", "--cam-pitch", "0"],
        about: "a sealed chamber lit by four emitters of four different colours, which is the \
                only capture in the set that can tell coloured block light from a warm tint \
                applied to a scalar one",
        crops: &[Crop {
            // The wall between the amber lamp and the azure one, which is the only surface in
            // the fixture receiving two differently coloured sources at once -- and therefore
            // the only one where three independent floods differ from any per-block tint
            // applied at read time, however the tint is chosen.
            name: "mixing-wall",
            content: Content::Land,
            x0: 0.38,
            y0: 0.26,
            x1: 0.62,
            y1: 0.62,
        }],
    },
    Vantage {
        name: "canopy",
        // **The sixteenth, added by batch 43, and the crop is the whole reason for it.**
        // Batch 38 carved leaf blocks into a 4^3 mask and left `LEAF_FILL` unswept; roadmap
        // 38d says what the sweep needs and the fixture did not have it -- *a canopy against
        // open sky*. Every other vantage shows leaves against ground or against other leaves,
        // where the carve reads as a speckle in a green mass; only a silhouette shows whether
        // a tree's outline breaks up at all, which is the thing the constant decides.
        //
        // A wooded crest at eye level, pitched just above the horizon so the ridge line cuts
        // across open sky. `--time 0.35` rather than the default because the canopy has to be
        // lit from the front: at a low sun the crest is a black cut-out against a bright sky
        // and the silhouette is all there is, which flatters the feature by hiding the
        // interior half of the trade.
        //
        // **Every argument here predates the batch**, so `bitexact --exe-b` can include this
        // row -- the hazard batch 38b hit with `--demo-glass`, whose flag the parent binary
        // could not parse, does not arise.
        args: &[
            "--cam-height",
            "8",
            "--cam-pitch",
            "14",
            "--cam-yaw",
            "300",
            "--time",
            "0.35",
        ],
        about: "a wooded crest against open sky: the only vantage where a canopy is a \
                silhouette rather than a mass, which is what `LEAF_FILL` decides and what \
                roadmap 38d had no crop for",
        crops: &[Crop {
            name: "skyline",
            // `Mixed`, and it has to be: the crop is half sky and half canopy by design, so
            // neither the `Sky` tag's "nothing moves under the water controls" nor `Land`'s
            // is the claim. What it is *for* is the leaf constant, and `CHECKS` is where a
            // claim about that belongs -- a crop tag cannot express "this many pixels are
            // trees".
            content: Content::Mixed,
            x0: 0.46,
            y0: 0.33,
            x1: 0.66,
            y1: 0.66,
        }],
    },
    Vantage {
        name: "offscreen-shadow",
        // **The twentieth, added by batch 64, and it is `sea-horizon`'s shape rather than
        // `canopy`'s**: a camera built because a *control* had nowhere to be ranked. Roadmap D3 is a
        // chunk outside the view frustum casting no shadow, so what it needs is a frame whose
        // sun is occluded by terrain the camera is not pointing at -- and at sixteen of the
        // nineteen cameras already here that is worth exactly zero pixels.
        //
        // **80 blocks up over the coast at yaw 265, which is 133 degrees off the sun's own
        // azimuth.** That angle is the whole camera: an occluder has to be further off the view
        // axis than the frustum's horizontal half-angle of 52 degrees before it is culled at
        // all, and it has to be within `shadow_dist`'s 220 blocks of the ground it shadows
        // before the *grid* is what carries it -- past that batch 37's light envelope does,
        // off a 512x512 height window that no frustum touches. The band between those two is
        // where this defect lives and it is narrower than it sounds: the same camera at
        // `--time 0.26` reads **0**, because a sun that low throws every shadow past 220 blocks
        // and into the envelope.
        //
        // `--time 0.32` over 0.28 by measurement and the two disagree about which number
        // matters. 0.28 moves **32,937** pixels and 0.32 moves **18,468**; over the crop 0.28
        // is MAE 0.22 at max delta 9 and 0.32 is **2.62 at max delta 27**. A count says the
        // feature fired and cannot say whether anybody can see it, which is batch 63's lesson
        // and is why this row is placed on the second pair of numbers.
        //
        // **Every argument predates the batch**, so this can be a `bitexact --exe-b` row from
        // the day it was added -- unlike `glass` and `lamps`, whose generators the parent
        // binary cannot parse.
        args: &[
            "--time",
            "0.32",
            "--cam-yaw",
            "265",
            "--cam-height",
            "80",
        ],
        about: "a coastal slope 133 degrees round from the sun: the only vantage where the \
                terrain casting the shadow is off screen, which is what `--no-offscreen-shadows` \
                needs and what no single still can otherwise show",
        crops: &[Crop {
            name: "shadowed-slope",
            // `Land` and checkably so: **0 pixels move** here under
            // `--no-water-refract --no-water-reflect --no-waves --water-absorb 0`, which is the
            // tag's own test, and the sea that fills the left two thirds of this frame is
            // entirely outside it. Placed by `compare --rect` on the difference itself -- it
            // holds 15,159 of the 18,468 pixels the control moves, at MAE 2.6165, against
            // 2.0816 for a wider rectangle that reaches the waterline and stops being `Land`.
            content: Content::Land,
            x0: 0.80,
            y0: 0.55,
            x1: 1.00,
            y1: 0.95,
        }],
    },
];

pub fn find(name: &str) -> Option<&'static Vantage> {
    VANTAGES.iter().find(|v| v.name == name)
}

/// Resolve a comma-separated selection, or every vantage when the selection is empty.
pub fn select(names: &str) -> Result<Vec<&'static Vantage>, String> {
    if names.trim().is_empty() {
        return Ok(VANTAGES.iter().collect());
    }
    names
        .split(',')
        .map(|n| find(n.trim()).ok_or_else(|| format!("no vantage named `{}`", n.trim())))
        .collect()
}

