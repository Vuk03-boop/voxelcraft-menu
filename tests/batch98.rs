//! Batch 98: the lookbook (`--lookbook DIR`) -- three flagship views against the whole
//! look-arm matrix, one labeled PNG, all of it executable from the CLI the user already
//! drives with flags. The questions no device answers, pinned here: the table shapes,
//! that a view is a camera and nothing more, that every combo actually mutates a config,
//! that BASE is genuinely the control, and that every fixed string the sheet paints is in
//! the font's charset (a tile whose label glyphs miss silently would lie to exactly the
//! reader the sheet is for).
//!
//! The renders themselves stay where renders live: the hardware round runs `--lookbook`.
//! Nothing in this file creates a device.

use voxelcraft::config::{parse_from, Config, Mode};
use voxelcraft::lookbook::{
    apply_view, covered, glyph, LookView, COLS, COMBOS, VIEWS, VIEW_CHANNELS,
};

fn sig(cfg: &Config) -> String {
    // Everything a combo is permitted to move, in one string, so "the combo did
    // something" is a property and not a diff on forty struct fields. G2's pair are
    // worldgen fields by design -- grass density and its sway are the subject the
    // MIDDAY-MEADOW camera was added to judge, so the sheet's one world-mutant tile is
    // named here rather than smuggled past `sanitized`.
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        cfg.sky_cool,
        cfg.water_look,
        cfg.foliage_rich,
        cfg.canopy_relief,
        cfg.grade_value(),
        cfg.grade_strength,
        cfg.fog.density,
        cfg.fog.falloff,
        cfg.fog.scatter,
        cfg.fog.g,
        cfg.godrays.strength,
        cfg.godrays.steps,
        cfg.soft_shadows,
        cfg.grass_dense,
        cfg.wind_sway,
    )
}

/// Views and combos are forever: the sheet is a deliverable people compare by name, so
/// its dimension is a property. Distinct labels, or a pasted duplicate reads as the same
/// arm twice. Batch 101 grew the matrix by one dial-tile (CINE-HALF) and the midday view
/// became MIDDAY-REEDS; 101j added MIDDAY-MEADOW after the hardware round measured G2
/// under every camera that didn't point at grass; 102a adds SOFT-SHADOWS, the penumbra
/// arm -- the names are the comparison's vocabulary, so this table names them all.
#[test]
fn the_matrix_is_four_by_fourteen_with_distinct_labels() {
    assert_eq!(VIEWS.len(), 4);
    assert_eq!(COMBOS.len(), 14);
    for (i, a) in COMBOS.iter().enumerate() {
        for b in &COMBOS[i + 1..] {
            assert_ne!(
                a.label, b.label,
                "two combos named {} -- the sheet cannot tell its arms apart",
                a.label
            );
        }
    }
    for (i, a) in VIEWS.iter().enumerate() {
        for b in &VIEWS[i + 1..] {
            assert_ne!(a.name, b.name, "two views named {}", a.name);
        }
    }
}

/// The BASE combo is the control, spelled as a property rather than a hope: applying it
/// changes nothing, and each other combo changes *something* in the permitted signature
/// -- a combo whose mutation is a no-op is a second control measuring nothing.
#[test]
fn base_moves_nothing_and_every_other_combo_moves_something() {
    let base = Config::default();
    let base_sig = sig(&base);
    for combo in &COMBOS {
        let mut c = Config::default();
        (combo.apply)(&mut c);
        if combo.label == "BASE" {
            assert_eq!(sig(&c), base_sig, "BASE must be the control");
        } else {
            assert_ne!(
                sig(&c),
                base_sig,
                "{} changes no look field against the control",
                combo.label
            );
        }
    }
}

/// A combo may only touch *look* fields -- the same ones the signature covers. Applying
/// every combo to the default and diffing every field the signature does **not** cover is
/// how a world mutation fails loudly (a lookbook that changes the world is not a
/// comparison of frames of one world). Pinned by construction: every permitted field is
/// in `sig`, and the Debug strings of the rests must not differ.
#[test]
fn combos_only_move_look_fields() {
    let base = format!("{:?}", sanitized(Config::default()));
    for combo in &COMBOS {
        let mut c = Config::default();
        (combo.apply)(&mut c);
        assert_eq!(
            format!("{:?}", sanitized(c)),
            base,
            "{} reaches past the look fields",
            combo.label
        );
    }
}

/// Strip the look fields out of a config so the Debug comparisons above each cover the
/// rest: world, camera, journal, resolution -- everything a combo must not author.
fn sanitized(mut c: Config) -> Config {
    c.sky_cool = false;
    c.water_look = false;
    c.foliage_rich = false;
    c.canopy_relief = false;
    c.grade_warm = false;
    c.grade_cine = false;
    c.grade_strength = 1.0;
    c.fog = Default::default();
    c.godrays = Default::default();
    // Batch 102: `soft_shadows` is this round's own mirror-miss -- `sig` gained it and
    // this list did not, and the fork's hardware round caught the red (`2bc0dba` is
    // that repair; this tree carries its text). `grass_dense`/`wind_sway` arrive with
    // the G2-PAIR repair: the sheet's one sanctioned world-mutant -- grass density is
    // what the tile exists to photograph, so the "look fields only" rule names it
    // rather than pretending it did not happen.
    c.grass_dense = false;
    c.wind_sway = false;
    c.soft_shadows = false;
    c
}

/// The views are the fixture's cameras: their argument lists are the fixture's own
/// vocabulary (sea-sunset and midday-reeds share coastline's yaw at two heights and two
/// times, sunrise-mountains is lod), and each one parses with no unknown arguments --
/// the batch-22 law for a camera somebody already trust-verified. They touch only
/// camera-plus-time channels: the whitelist is the claim, asserted against the table itself.
#[test]
fn the_views_are_cameras_and_nothing_more() {
    // The whitelist is the module's own table, read through the public name -- the
    // module's doc claims this pin; a second list typed into the test would be two
    // mirrors kept in sync by eye, and the round's dead-code warning was the table's
    // way of saying nothing looked at it.
    const WHITELIST: &[&str] = &VIEW_CHANNELS;
    for view in &VIEWS {
        let mut i = 0;
        while i < view.args.len() {
            let flag = view.args[i];
            assert!(
                WHITELIST.contains(&flag),
                "view `{}` reaches past the camera: `{}` -- a lookbook compares frames \
                 of one world",
                view.name,
                flag
            );
            i += 2; // every whitelisted flag takes one value
        }
        assert_eq!(i, view.args.len(), "odd argument count in {}", view.name);
        // And parse cleanly under the real parser; a renamed flag fails here, not as a
        // default-camera capture at the hardware round.
        let mut c = Config::default();
        assert!(
            apply_view(&mut c, view),
            "view `{}` uses arguments the parser no longer knows",
            view.name
        );
    }
    // The two module constants the views lean on have not drifted: view applications move
    // exactly the channels the whitelist covers.
    let mut c = Config::default();
    assert!(apply_view(&mut c, &VIEWS[0]));
    assert!((c.time_of_day - 0.74).abs() < 1e-6, "SEA-SUNSET is a sunset");
    assert_eq!(c.cam_yaw, 143.0, "SEA-SUNSET is coastline's camera");
    let mut c = Config::default();
    assert!(apply_view(&mut c, &VIEWS[2]));
    assert!((c.time_of_day - 0.50).abs() < 1e-6, "MIDDAY-REEDS is noon");
    assert_eq!(c.cam_height, 2.0, "the reeds view stands where the references stand");
    assert_eq!(c.cam_yaw, 143.0, "the reeds view is coastline's own yaw, lowered");
}

/// Everything fixed that the sheet paints is in the font's charset: every combo label,
/// every view name. The coverage helper is the test's own subject too -- it rejects a
/// lowercase letter and an underscore, and the fallback glyph exists for the day a rename
/// sneaks past anyway.
#[test]
fn the_font_covers_every_fixed_label() {
    for combo in &COMBOS {
        assert!(covered(combo.label), "{} leaves the charset", combo.label);
    }
    for view in &VIEWS {
        assert!(covered(view.name), "{} leaves the charset", view.name);
    }
    assert!(!covered("sky-cool"), "lowercase must not paint");
    assert!(!covered("ALL_FIVE"), "underscores must not paint");
    assert!(covered("BASE"));
    assert!(covered("G2-PAIR"));
    assert!(covered("FOG-LOW"));
    let unknown = glyph('~');
    assert_eq!(unknown[0], 0x1F, "the fallback glyph is the '?'");
}

/// The compose itself, GPU-free: stack three fabricated tiles through compose_group and
/// compose_sheet and verify the layout arithmetic is the arithmetic it claims (one file
/// per RUN was the user's ask, but the math of labels and grids is exactly what a wrong
/// number turns to sewage in). A 4x4 checkerboard at 8x6 under SCALE 2 gives a 4x3 tile:
/// every downsampled pixel is the mean of its 2x2.
#[test]
fn the_compose_lays_out_and_downsamples_what_it_claims() {
    use voxelcraft::lookbook::{compose_group, compose_sheet, GROUP_H, LABEL_H, SCALE};
    let view = LookView {
        name: "TEST",
        about: "",
        args: &[],
    };
    let tiles: Vec<voxelcraft::lookbook::Tile> = (0..4)
        .map(|n| voxelcraft::lookbook::Tile {
            label: format!("T{n}"),
            width: 8,
            height: 6,
            pixels: vec![n as u8; 8 * 6 * 4],
        })
        .collect();
    let (w, h, px) = compose_group(&view, &tiles);
    assert_eq!(
        w,
        COLS * (8 / SCALE),
        "four finite tiles at SCALE 2 stand four abreast"
    );
    assert_eq!(h, GROUP_H + LABEL_H + 3, "one tile row under one group strip");
    // The sheet buffer is exactly addressed.
    assert_eq!(px.len() as u32, w * h * 4);
    // Tile 0's first pixel sits directly below the first label strip, so it is the mean
    // of tile 0's top-left 2x2 block -- a fully zero tile, so black. Batch 98's first
    // hardware round read 204 (the label shade) at this address, and the sandbox's own
    // stale-binary suspicion was WRONG: the fork found the defect -- `put_px` let
    // past-edge x spill into the next row via the linear index, and on a sheet exactly
    // four tiles wide the rightmost label strip's text starts at x == w, so its stamp
    // wrapped two rows down into tile 0's data. Engine-side fix: put_px clips. The
    // offsets intentionally re-derive from the layout constants so a future sheet
    // redesign moves the expectation with the module instead of corrupting the check.
    let first = ((GROUP_H + LABEL_H) * w) as usize * 4;
    assert_eq!(
        &px[first..first + 3],
        &[0u8; 3],
        "tile 0's first downsampled pixel; 204 here is label ink wrapping past the sheet edge"
    );
    // Tile 2's first pixel after downsample: the uniform tile value survives the mean.
    let t2 = ((GROUP_H + LABEL_H) * w + 2 * (8 / SCALE)) as usize * 4;
    assert_eq!(
        &px[t2..t2 + 3],
        &[2u8; 3],
        "a uniform tile survives 2x2 exactly"
    );
    // Stacking three groups gives back their heights' sum and one shared width.
    let a = compose_group(&view, &tiles);
    let b = compose_group(&view, &tiles);
    let (sw, sh, spx) = compose_sheet(&[a.clone(), b.clone(), a]);
    assert_eq!(sw, w);
    assert_eq!(sh, h * 3);
    assert_eq!(spx.len() as u32, sw * sh * 4);
}

/// The mode parses: `--lookbook out` lands in `Mode::Lookbook`, and the flag shows in
/// `--help` (the docs.rs census handles that mechanically, but the parse twin of the
/// claim lives beside the table the mode drives).
#[test]
fn the_mode_parses() {
    let owned: Vec<String> = vec!["--lookbook".into(), "out".into()];
    let (_cfg, mode, unknown) = parse_from(&owned);
    assert!(unknown.is_empty());
    assert!(
        matches!(mode, Mode::Lookbook { ref dir } if dir == "out"),
        "--lookbook DIR lands its own mode"
    );
}

/// And the shape of the module contract: the driver never renders a reference frame --
/// the clamp lives in headless, spelled once, so a `--reference` handed to a lookbook run
/// does not silently swap its own tiles for converged ones.
#[test]
fn the_driver_forbids_the_reference_branch() {
    let src = include_str!("../src/headless.rs");
    assert!(
        src.contains("c.reference = 0;"),
        "the lookbook clamps the reference branch closed -- a sheet of reference frames \
         answers no question the user asked"
    );
    assert!(
        src.contains("c.hud = false;"),
        "no HUD: the sheet labels its own tiles"
    );
}

