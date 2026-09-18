//! The fixture's own tests: batch 22's new test, and the one that keeps the table honest.
//!
//! Nothing here touches wgpu -- nothing in `tests/` does, which is why batch 11's capture
//! hashes went into `docs/` rather than into a test file. What *can* be tested without a GPU
//! turns out to be most of what goes wrong with a fixture: an argument list that no longer
//! parses, a crop that has drifted off the frame, and a metric that does not compute what its
//! name says. The parts that need pixels are `harness validate`, which is a command and is run
//! at the start of a session rather than in the suite.

use voxelcraft::config::{parse_from, Mode};
use voxelcraft::harness::metric::{self, Img, Rect};
use voxelcraft::harness::vantage::{self, Content};
use voxelcraft::harness;

/// **The load-bearing one.** Every vantage in the table has to be expressible in today's
/// command line.
///
/// Before batch 22 a vantage was a sentence in a document and nothing could notice when it
/// stopped being runnable. `parse_from` hands back what it did not recognise, so a flag
/// renamed out from under the fixture fails here instead of silently producing a capture from
/// the default camera -- which is a frame, and looks like one, and is not the vantage anybody
/// asked for.
#[test]
fn every_vantage_parses() {
    for v in vantage::VANTAGES {
        let args: Vec<String> = v.args.iter().map(|s| s.to_string()).collect();
        let (_cfg, mode, unknown) = parse_from(&args);
        assert!(
            unknown.is_empty(),
            "vantage `{}` uses arguments the parser does not know: {unknown:?}",
            v.name
        );
        assert!(
            matches!(mode, Mode::Run),
            "vantage `{}` sets a mode; a vantage is a camera, not a command",
            v.name
        );
    }
}

/// A vantage's arguments have to *do* something, or the name is describing the default camera
/// under another title. Two vantages that parse to the same configuration are one vantage.
#[test]
fn vantages_are_distinct() {
    let mut seen: Vec<(&str, String)> = Vec::new();
    for v in vantage::VANTAGES {
        assert!(
            !seen.iter().any(|(n, _)| *n == v.name),
            "two vantages named `{}`",
            v.name
        );
        let args: Vec<String> = v.args.iter().map(|s| s.to_string()).collect();
        let (cfg, _, _) = parse_from(&args);
        // Everything the vantage table is allowed to vary, as one string.
        let sig = format!(
            "{:?} {} {} {} {} {} {} {} {}",
            cfg.time_of_day,
            cfg.cam_height,
            cfg.cam_yaw,
            cfg.cam_pitch,
            cfg.cam_submerge,
            cfg.clouds.cover,
            cfg.demo_edits,
            cfg.sea_level,
            cfg.freeze_time,
        );
        if let Some((other, _)) = seen.iter().find(|(_, s)| *s == sig) {
            panic!("vantages `{}` and `{}` are the same camera", other, v.name);
        }
        seen.push((v.name, sig));
    }
    assert_eq!(seen.len(), vantage::VANTAGES.len());
}

/// Crops are fractions, and have to stay inside the frame and be non-empty at both of the
/// capture sizes this project has actually used.
#[test]
fn crops_are_well_formed() {
    for v in vantage::VANTAGES {
        for c in v.crops {
            assert!(
                (0.0..=1.0).contains(&c.x0)
                    && (0.0..=1.0).contains(&c.y0)
                    && (0.0..=1.0).contains(&c.x1)
                    && (0.0..=1.0).contains(&c.y1),
                "{}/{}: a crop is in fractions of the frame",
                v.name,
                c.name
            );
            assert!(
                c.x0 < c.x1 && c.y0 < c.y1,
                "{}/{}: empty or inverted",
                v.name,
                c.name
            );
            for (w, h) in [
                (vantage::STD_WIDTH, vantage::STD_HEIGHT),
                (960, 540),
                (1920, 1080),
            ] {
                let r = c.rect(w, h);
                assert!(r.x1 <= w && r.y1 <= h, "{}/{} leaves {w}x{h}", v.name, c.name);
                assert!(
                    r.pixels() >= 4096,
                    "{}/{} is {} pixels at {w}x{h}, too small to average over",
                    v.name,
                    c.name,
                    r.pixels()
                );
            }
            assert!(
                v.crops.iter().filter(|o| o.name == c.name).count() == 1,
                "{} has two crops named {}",
                v.name,
                c.name
            );
        }
    }
}

/// Every content tag `harness validate` knows how to check has at least one crop wearing it,
/// and every water vantage has a water crop. A tag nothing uses is a check nothing runs.
#[test]
fn tagged_crops_exist() {
    let all: Vec<Content> = vantage::VANTAGES
        .iter()
        .flat_map(|v| v.crops.iter().map(|c| c.content))
        .collect();
    for t in [Content::Water, Content::Land, Content::Sky] {
        assert!(all.contains(&t), "no crop is tagged {}", t.label());
    }
    for name in ["coastline", "open-sea", "shore", "lattice", "terraces"] {
        let v = vantage::find(name).expect("water vantage missing from the table");
        assert!(
            v.crops.iter().any(|c| c.content == Content::Water),
            "{name} is a water vantage with no water crop to score"
        );
    }
}

#[test]
fn selection_resolves() {
    assert_eq!(vantage::select("").unwrap().len(), vantage::VANTAGES.len());
    let two = vantage::select("cave, night").unwrap();
    assert_eq!(two.len(), 2);
    assert_eq!(two[0].name, "cave");
    assert!(vantage::select("nowhere").is_err());
}

// ------------------------------------------------------------------ the metrics themselves

fn flat(w: u32, h: u32, rgb: [u8; 3]) -> Img {
    let mut i = Img::new(w, h);
    for p in i.px.as_chunks_mut::<4>().0 {
        p[0] = rgb[0];
        p[1] = rgb[1];
        p[2] = rgb[2];
        p[3] = 255;
    }
    i
}

#[test]
fn difference_metrics() {
    let a = flat(64, 64, [10, 20, 30]);
    let b = flat(64, 64, [14, 20, 30]);
    let r = Rect::full(64, 64);
    assert_eq!(metric::mae(&a, &a, r), 0.0);
    assert_eq!(metric::pixels_differing(&a, &a, r), 0);
    assert_eq!(metric::max_delta(&a, &a, r), 0);
    // One channel of three moved by 4, so the mean over RGB is 4/3.
    assert!((metric::mae(&a, &b, r) - 4.0 / 3.0).abs() < 1e-9);
    assert_eq!(metric::max_delta(&a, &b, r), 4);
    assert_eq!(metric::pixels_differing(&a, &b, r), 64 * 64);
    // And a crop really is a crop: half the width, half the pixels.
    let half = Rect {
        x0: 0,
        y0: 0,
        x1: 32,
        y1: 64,
    };
    assert_eq!(metric::pixels_differing(&a, &b, half), 32 * 64);
}

#[test]
fn speckle_is_zero_on_flat_and_known_on_a_checkerboard() {
    let r = Rect::full(64, 64);
    assert_eq!(metric::speckle(&flat(64, 64, [128, 128, 128]), r), 0.0);

    // A one-pixel checkerboard between 100 and 200. The 3x3 mean of a centre pixel is its own
    // value plus four diagonal neighbours of the same value and four edge neighbours of the
    // other, so it is (5a + 4b) / 9 and the residual is |a - that| = 4|a - b| / 9.
    let mut c = Img::new(64, 64);
    for y in 0..64u32 {
        for x in 0..64u32 {
            let v = if (x + y) % 2 == 0 { 100 } else { 200 };
            let i = ((y * 64 + x) * 4) as usize;
            c.px[i] = v;
            c.px[i + 1] = v;
            c.px[i + 2] = v;
            c.px[i + 3] = 255;
        }
    }
    // Scored over the interior, so the clamped edge neighbours do not enter.
    let inner = Rect {
        x0: 1,
        y0: 1,
        x1: 63,
        y1: 63,
    };
    let want = 4.0 * 100.0 / 9.0;
    assert!(
        (metric::speckle(&c, inner) - want).abs() < 1e-9,
        "{} against {want}",
        metric::speckle(&c, inner)
    );
}

#[test]
fn rg_std_sees_path_length_and_not_brightness() {
    let r = Rect::full(64, 64);
    // A constant R - G has zero spread however bright the image is: that is the property the
    // metric is chosen for, since a material change moves both channels together far more than
    // it moves their difference.
    let mut ramp = Img::new(64, 64);
    for y in 0..64u32 {
        for x in 0..64u32 {
            let i = ((y * 64 + x) * 4) as usize;
            ramp.px[i] = 40 + x as u8;
            ramp.px[i + 1] = 20 + x as u8;
            ramp.px[i + 2] = 90;
            ramp.px[i + 3] = 255;
        }
    }
    assert!(metric::rg_std(&ramp, r) < 1e-9);

    // Alternate columns of R - G = 0 and R - G = 20: mean 10, so the deviation is 10.
    let mut stripes = Img::new(64, 64);
    for y in 0..64u32 {
        for x in 0..64u32 {
            let i = ((y * 64 + x) * 4) as usize;
            stripes.px[i] = if x % 2 == 0 { 50 } else { 70 };
            stripes.px[i + 1] = 50;
            stripes.px[i + 2] = 50;
            stripes.px[i + 3] = 255;
        }
    }
    assert!((metric::rg_std(&stripes, r) - 10.0).abs() < 1e-9);
}

/// The blit encodes by hand when the target has no hardware sRGB, so everything a capture
/// holds is encoded and everything averaged has to be decoded first. A round trip that lost
/// values would quietly bias every reference.
#[test]
fn srgb_round_trips() {
    for v in 0..=255u8 {
        assert_eq!(metric::linear_to_srgb(metric::srgb_to_linear(v)), v);
    }
    assert_eq!(metric::srgb_to_linear(0), 0.0);
    assert!((metric::srgb_to_linear(255) - 1.0).abs() < 1e-6);
}

/// Against the published vectors, because `docs/sky.md` records nine of these and
/// a hash that is merely self-consistent could not be compared with them.
#[test]
fn sha256_matches_the_standard_vectors() {
    assert_eq!(
        metric::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        metric::sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        metric::sha256_hex(&[b'a'; 1_000_000]),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}
/// Every flag in the composition table has to be one the parser knows -- **and this is the
/// cheap half of a guard whose expensive half is a six-minute command.**
///
/// `harness implies` renders 112 frames to check eighteen claims. A misspelled flag in that
/// table does not make it fail: `parse_args` ignores what it cannot read and exits 0, so side
/// B renders exactly what side A rendered and the claim passes, vacuously. `harness::render`
/// refuses such a capture at run time, which is the backstop; catching it here is free and
/// happens before anyone waits six minutes for a table of meaningless zeros.
#[test]
fn every_implication_parses() {
    for imp in harness::IMPLIES {
        for (which, set) in [("base", imp.base), ("also", imp.also)] {
            let args: Vec<String> = set.iter().map(|s| s.to_string()).collect();
            let (_cfg, mode, unknown) = parse_from(&args);
            assert!(
                unknown.is_empty(),
                "implication `{}` + `{}` has a {which} the parser does not know: {unknown:?}",
                imp.base.join(" "),
                imp.also.join(" ")
            );
            assert!(
                matches!(mode, Mode::Run),
                "implication `{}` + `{}` sets a mode in its {which}; these are flags, not commands",
                imp.base.join(" "),
                imp.also.join(" ")
            );
        }
    }
}

/// Every vantage a claim names has to exist, and a claim has to name at least one.
///
/// A claim with no vantages is not a weak check, it is zero checks that print as a pass.
#[test]
fn every_implication_names_live_vantages() {
    for imp in harness::IMPLIES {
        let names: Vec<&str> = imp.only.split(',').map(str::trim).collect();
        assert!(
            !names.is_empty() && !imp.only.is_empty(),
            "implication `{}` + `{}` names no vantage",
            imp.base.join(" "),
            imp.also.join(" ")
        );
        for n in names {
            assert!(
                vantage::find(n).is_some(),
                "implication `{}` + `{}` names vantage `{n}`, which does not exist",
                imp.base.join(" "),
                imp.also.join(" ")
            );
        }
    }
}

/// `also` has to add something `base` does not already say, or the claim is true by string
/// equality rather than by anything about the engine.
#[test]
fn implications_add_a_flag_not_already_in_the_base() {
    for imp in harness::IMPLIES {
        assert!(
            !imp.also.is_empty(),
            "implication on `{}` adds nothing",
            imp.base.join(" ")
        );
        // Flag tokens only. `--cloud-shadow 0` and `--godray-strength 0` share the *value*
        // `0` as a separate element, and comparing every token makes that pair look like a
        // repetition when it is two different flags -- which is exactly the pair carrying the
        // table's only directional claim.
        for a in imp.also.iter().filter(|a| a.starts_with("--")) {
            assert!(
                !imp.base.contains(a),
                "implication `{}` + `{}` repeats `{a}`, so it cannot fail",
                imp.base.join(" "),
                imp.also.join(" ")
            );
        }
    }
}

/// **The one that keeps the table able to fail at all.** Both verdicts have to be represented.
///
/// A composition table naturally fills up with `Nothing` rows, because that is the shape of
/// the claim being audited -- and a table that only ever expects `0 pixels differing` cannot
/// tell a true claim from a fixture that has stopped rendering. This is the same rule
/// `CHECKS` encodes for the metrics, applied to the control table: at least one row has to
/// expect the frame to move, or the zeros mean nothing.
#[test]
fn the_composition_table_can_fail() {
    let nothing = harness::IMPLIES
        .iter()
        .filter(|i| i.expect == harness::Composes::Nothing)
        .count();
    let something = harness::IMPLIES
        .iter()
        .filter(|i| i.expect == harness::Composes::Something)
        .count();
    assert!(nothing > 0, "no `Nothing` claims: nothing is being audited");
    assert!(
        something > 0,
        "no `Something` claims: a table of `Nothing` rows passes just as well when the \
         fixture has stopped rendering, which is exactly what batch 22b found `bitexact` \
         unable to distinguish"
    );
}

// ---------------------------------------------------------------- batch 24: the bench parser

/// A real `--bench-frames` stdout, including the adapter banner that also begins with `gpu`.
const BENCH_STDOUT: &str = "gpu: NVIDIA GeForce RTX 3050 Laptop GPU
generating world...
  904 chunks in 0.4s

--- 1920x1080  hi-z off  taa on  106 chunks ---
  wall  median 12.72ms  (79 fps)   p99 14.58ms
  gpu   select 0.13  march 2.26  hiz 0.00  recover 0.00  march2 0.00  resolve 9.20  taa 0.46
  gpu   total 12.04ms
  pairs marched 214189  deferred 0

--- 1920x1080  hi-z on  taa on  106 chunks ---
  wall  median 11.90ms  (84 fps)   p99 14.12ms
  gpu   select 0.13  march 1.00  hiz 0.10  recover 0.02  march2 0.05  resolve 9.22  taa 0.46
  gpu   total 10.97ms
  pairs marched 76012  deferred 138236
";

/// The bench parser reads both blocks, and is not fooled by the adapter banner.
///
/// `gpu: NVIDIA GeForce RTX 3050 Laptop GPU` is the first line of every bench run and also
/// begins with `gpu` -- the first version of this parser matched it, tried to read a `select`
/// column out of a device name and failed loudly, which is the good outcome. The bad outcome
/// was available: a parser that treated a missing column as `0.0` would have reported the
/// adapter line as a block of zeroes and made every A/B against it a large improvement.
#[test]
fn bench_parses_both_blocks_and_ignores_the_adapter_banner() {
    let r = harness::bench::parse(BENCH_STDOUT).expect("parse");
    // hi-z off, in `PASSES` order: select march hiz recover march2 resolve taa total
    assert_eq!(r.off, [0.13, 2.26, 0.00, 0.00, 0.00, 9.20, 0.46, 12.04]);
    assert_eq!(r.on, [0.13, 1.00, 0.10, 0.02, 0.05, 9.22, 0.46, 10.97]);
    assert_eq!(r.on[harness::bench::RESOLVE], 9.22);
}

/// Anything that is not two complete blocks is an error, never a zero.
///
/// This is the half that matters. A fixture that quietly returned `0.0` for a pass it could
/// not find would turn a crashed or truncated run into the most attractive result a bench can
/// produce, and the rest of the sweep would average it in without comment.
#[test]
fn bench_refuses_a_partial_run() {
    assert!(harness::bench::parse("").is_err(), "no blocks at all");
    assert!(
        harness::bench::parse("gpu: NVIDIA GeForce RTX 3050 Laptop GPU").is_err(),
        "the banner alone is not a run"
    );
    let one_block = BENCH_STDOUT
        .split("--- 1920x1080  hi-z on")
        .next()
        .unwrap();
    assert!(
        harness::bench::parse(one_block).is_err(),
        "one block is half a run, and half a run must not score"
    );
    let missing_column = BENCH_STDOUT.replace("resolve 9.22", "9.22");
    assert!(
        harness::bench::parse(&missing_column).is_err(),
        "a renamed or dropped column must fail rather than read as zero"
    );
}

/// The paired difference is paired, and its standard error falls with the round count.
///
/// Batch 24 exists because "interleaved" did not specify a protocol. The arithmetic below is
/// the part of that protocol a test can hold: the delta is the mean of the per-round
/// differences and not the difference of the means -- equal in expectation, different in
/// variance, and the pairing is the entire reason the command can see 0.02 ms through a drift
/// of 0.18.
#[test]
fn paired_delta_is_paired() {
    let mk = |resolve: f32| {
        let mut r = harness::bench::Run::default();
        r.on[harness::bench::RESOLVE] = resolve;
        r
    };
    // A drifts 9.0 -> 9.3 across the session; B is always exactly 0.1 above its own partner.
    // An unpaired comparison of the means would still give +0.1 here, so the discriminating
    // property is the *standard error*: paired it is 0, unpaired it would carry the drift.
    let p = harness::bench::Paired {
        a: vec![mk(9.0), mk(9.1), mk(9.2), mk(9.3)],
        b: vec![mk(9.1), mk(9.2), mk(9.3), mk(9.4)],
    };
    let c = p.cell(true, harness::bench::RESOLVE);
    assert!((c.delta - 0.1).abs() < 1e-4, "delta was {}", c.delta);
    assert!(c.stderr < 1e-4, "a constant offset has no spread, got {}", c.stderr);
    assert_eq!(c.positive, 4);
    assert_eq!(p.rounds(), 4);
}

/// One frame-sized image whose detail either repeats on a lattice or does not, with nothing
/// else in it.
///
/// `period` is the lattice spacing in pixels. With `shuffle` off every cell holds the same
/// noise, which is the in-phase defect in its purest form; with it on each cell holds its own,
/// which is what a permutation or a variant set is trying to approximate. Both images have the
/// **same** amount of detail at the same scale -- that is the point of the pair, and it is the
/// difference `speckle` cannot see.
fn lattice(period: u32, shuffle: bool) -> Img {
    let mut img = Img::new(vantage::STD_WIDTH, vantage::STD_HEIGHT);
    for y in 0..img.h {
        for x in 0..img.w {
            let cell = if shuffle {
                (y / period) * 4096 + (x / period)
            } else {
                0
            };
            // `cell * 1_000_003` wraps for cells past ~(90 * 4096), and debug builds
            // panic on an overflowing multiply where a release build's bench arithmetic
            // wraps silently. **Wrap deliberately, in code, so the two profiles agree
            // bit-for-bit** -- the sandbox's debug suite and the bench's release gates
            // must build the same lattice, or "it passed here" says nothing about there.
            let key = cell
                .wrapping_mul(1_000_003)
                .wrapping_add((y % period) * 67 + (x % period));
            let mut h = key.wrapping_mul(0x7feb_352d);
            h ^= h >> 15;
            h = h.wrapping_mul(0x846c_a68b);
            h ^= h >> 16;
            let v = (h >> 24) as u8;
            let p = ((y * img.w + x) * 4) as usize;
            img.px[p] = v;
            img.px[p + 1] = v;
            img.px[p + 2] = v;
            img.px[p + 3] = 255;
        }
    }
    img
}

/// **The rule of `CHECKS`, applied to `coherence` without a renderer in the way.**
///
/// The row in `CHECKS` is worth 1.0755x and no more, because at `default/slope` the lattice
/// the metric finds is the terrain's staircase and the texture's phase together and the
/// control only moves one of them. That is an honest reading of a frame and a poor test of the
/// *instrument*: a metric that had gone blind would still be worth some number there and
/// nobody could say which. Here the two images differ in exactly one thing, so the separation
/// is the metric's own and a regression in it cannot hide behind the picture.
///
/// **Batch 39 is the demonstration, and it is why this test carries the fitness claim now.**
/// That row was 1.2053x when it was written and fell to 1.0755x without one line of the metric
/// changing -- batch 38 carved the tree canopy, and four fifths of what the control had been
/// moving was leaf cubes sampling their layer in phase. A known-positive read off a frame is
/// only as durable as the frame's contents. This one has no frame in it.
///
/// The negative is the half that matters. Both images carry the same noise at the same scale,
/// so `speckle` cannot tell them apart and this has to.
#[test]
fn coherence_separates_a_lattice_from_the_same_noise_unlatticed() {
    const PERIOD: u32 = 20;
    let rect = Rect {
        x0: 200,
        y0: 200,
        x1: 456,
        y1: 456,
    };
    let tiled = metric::coherence(&lattice(PERIOD, false), rect);
    let shuffled = metric::coherence(&lattice(PERIOD, true), rect);
    println!(
        "lattice {:.4} @ {:?} over {} tiles, unlatticed {:.4} @ {:?}",
        tiled.peak, tiled.lag, tiled.tiles, shuffled.peak, shuffled.lag
    );
    assert!(tiled.tiles >= 4, "the crop held {} tiles", tiled.tiles);
    assert!(
        tiled.peak > 0.9,
        "a field that reproduces itself exactly every {PERIOD} pixels scores {:.4}; the metric \
         cannot see a repeat at all",
        tiled.peak
    );
    assert!(
        shuffled.peak < 0.4,
        "unlatticed noise scores {:.4}, so the number above is not about the lattice",
        shuffled.peak
    );
    // The lag is the diagnostic half, and a peak at the wrong place would mean the metric had
    // found the right amount of structure in the wrong period.
    let (dx, dy) = tiled.lag;
    assert!(
        dx.unsigned_abs() % PERIOD == 0 && dy.unsigned_abs() % PERIOD == 0,
        "the lattice peaked at {:?}, which is not a multiple of its own period",
        tiled.lag
    );
    // And the pair `speckle` cannot separate, which is why this metric exists. Both images are
    // the same noise at the same scale.
    let sa = metric::speckle(&lattice(PERIOD, false), rect);
    let sb = metric::speckle(&lattice(PERIOD, true), rect);
    assert!(
        (sa - sb).abs() / sa < 0.05,
        "speckle reads {sa:.4} and {sb:.4} on the pair, so this pair does not demonstrate what \
         it is quoted for"
    );
}

/// A crop with nothing in it reads 0 rather than whatever the rounding left behind.
///
/// A correlation is a ratio, and the denominator here is the detail the high-pass leaves. On
/// a flat region there is none, and a metric that divided anyway would report a number between
/// -1 and 1 drawn from the last bits of an empty sum -- which is exactly the kind of reading a
/// later session quotes without knowing it is noise.
#[test]
fn coherence_of_a_flat_crop_is_zero() {
    let mut img = Img::new(vantage::STD_WIDTH, vantage::STD_HEIGHT);
    img.px.iter_mut().for_each(|b| *b = 128);
    let k = metric::coherence(
        &img,
        Rect {
            x0: 200,
            y0: 200,
            x1: 456,
            y1: 456,
        },
    );
    assert_eq!(k.peak, 0.0, "a flat crop scored {:.6}", k.peak);
}




