use voxelcraft::config::{parse_from, Config, Mode};
use voxelcraft::lookbook::{
    apply_view, covered, glyph, LookView, COLS, COMBOS, VIEWS, VIEW_CHANNELS,
};

fn sig(cfg: &Config) -> String {

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

    c.grass_dense = false;
    c.wind_sway = false;
    c.soft_shadows = false;
    c
}

#[test]
fn the_views_are_cameras_and_nothing_more() {

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
            i += 2;
        }
        assert_eq!(i, view.args.len(), "odd argument count in {}", view.name);

        let mut c = Config::default();
        assert!(
            apply_view(&mut c, view),
            "view `{}` uses arguments the parser no longer knows",
            view.name
        );
    }

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

    assert_eq!(px.len() as u32, w * h * 4);

    let first = ((GROUP_H + LABEL_H) * w) as usize * 4;
    assert_eq!(
        &px[first..first + 3],
        &[0u8; 3],
        "tile 0's first downsampled pixel; 204 here is label ink wrapping past the sheet edge"
    );

    let t2 = ((GROUP_H + LABEL_H) * w + 2 * (8 / SCALE)) as usize * 4;
    assert_eq!(
        &px[t2..t2 + 3],
        &[2u8; 3],
        "a uniform tile survives 2x2 exactly"
    );

    let a = compose_group(&view, &tiles);
    let b = compose_group(&view, &tiles);
    let (sw, sh, spx) = compose_sheet(&[a.clone(), b.clone(), a]);
    assert_eq!(sw, w);
    assert_eq!(sh, h * 3);
    assert_eq!(spx.len() as u32, sw * sh * 4);
}

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
