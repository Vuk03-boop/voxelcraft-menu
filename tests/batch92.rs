use voxelcraft::config::{parse_from, Config};
use voxelcraft::render::{self, FLAG_HI_TINT_BALANCE};
use voxelcraft::shaft::ShaftField;

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

#[test]
fn a8_1_default_locks_the_shipping_constant() {
    assert_eq!(ShaftField::new().texel(), voxelcraft::shaft::TEXEL);
    let cfg = Config::default();
    assert_eq!(cfg.shaft_texel, voxelcraft::shaft::TEXEL);
}

#[test]
fn a8_1_flag_parses_and_clamps() {
    assert_eq!(cfg_of(&["--shaft-texel", "2"]).shaft_texel, 2);
    assert_eq!(cfg_of(&["--shaft-texel", "0"]).shaft_texel, 1);
    assert_eq!(cfg_of(&["--shaft-texel", "999"]).shaft_texel, 64);
}

#[test]
fn a8_1_the_shader_never_knew_a_constant() {
    let src = render::shader_source();

    assert!(
        src.contains("span * frame.shaft_texel *"),
        "the scan's climb reads the uniform -- this knob's whole premise"
    );
    assert!(
        src.contains("/ frame.shaft_texel - 0.5"),
        "the sampler's window address reads the uniform"
    );
}

#[test]
fn a8_1_the_fill_quantises_at_the_knob() {
    let gen = voxelcraft::worldgen::WorldGen::new(41);
    let sun = [0.4f32, 0.6, 0.5];
    for texel in [2i32, 4, 8] {
        let mut field = ShaftField::with_texel(texel);
        field.update(&gen, 0.0, 0.0, 0.0);

        let origin = field.origin_world();
        assert_eq!(
            origin[0],
            (0.0f32 / texel as f32).floor() - 256.0 * texel as f32,
            "texel {texel}: the anchor arithmetic still runs in texels"
        );

        let mut checked = 0u32;
        for zz in (0..512).step_by(64) {
            for xx in (0..512).step_by(32) {
                let wx = (origin[0] + (xx + 1) as f32 * texel as f32) as i32 - texel / 2;
                let wz = (origin[1] + (zz + 1) as f32 * texel as f32) as i32 - texel / 2;
                let want = gen.height(wx, wz) as f32;
                let got = field.heights()[zz * 512 + xx];
                assert_eq!(
                    got, want,
                    "texel {texel}, texel ({xx},{zz}): the fill read somewhere the \
                     knob did not send it"
                );
                checked += 1;
            }
        }
        assert!(checked > 100);

        let horiz = (sun[0] * sun[0] + sun[2] * sun[2]).sqrt().max(1e-3);
        let dir = [sun[0] / horiz, sun[2] / horiz];
        let slope = sun[1] / horiz;
        let gather = |tx: usize, tz: usize, k_texel: i32| -> f32 {
            let mut best = field.heights()[tz * 512 + tx];
            for m in 1..=256usize {
                let sx = tx as f32 + dir[0] * m as f32;
                let sz = tz as f32 + dir[1] * m as f32;
                let hi = 511.0f32;
                let cx = sx.clamp(0.0, hi);
                let cz = sz.clamp(0.0, hi);
                let (x0, z0) = (cx.floor() as usize, cz.floor() as usize);
                let (fx, fz) = (cx - x0 as f32, cz - z0 as f32);
                let (x1, z1) = ((x0 + 1).min(511), (z0 + 1).min(511));
                let hsz = field.heights();
                let a = hsz[z0 * 512 + x0] + (hsz[z0 * 512 + x1] - hsz[z0 * 512 + x0]) * fx;
                let b = hsz[z1 * 512 + x0] + (hsz[z1 * 512 + x1] - hsz[z1 * 512 + x0]) * fx;
                let h = a + (b - a) * fz;
                best = best.max(h - m as f32 * k_texel as f32 * slope);
            }
            best
        };
        for (tx, tz) in [(100usize, 100usize), (64, 320), (400, 17)] {
            let direct = field.envelope_reference(sun, tx, tz);
            let at_knob = gather(tx, tz, texel);
            assert!(
                (direct - at_knob).abs() < 1e-3,
                "texel {texel}, column ({tx},{tz}): the reference does not gather at \
                 the field's own quantisation ({direct} vs {at_knob}) -- that is batch \
                 35's scan-vs-gather test validating a field nobody asked for"
            );
        }
        if texel != 4 {

            let mut moved = false;
            'grid: for tz in (8..504).step_by(11) {
                for tx in (8..504).step_by(13) {
                    if (field.envelope_reference(sun, tx, tz) - gather(tx, tz, 4)).abs()
                        > 1e-2
                    {
                        moved = true;
                        break 'grid;
                    }
                }
            }
            assert!(
                moved,
                "texel {texel}: the knob and the old constant gather the same \
                 envelope at every column of a 512x512 grid -- the sweep would be \
                 invisible"
            );
        }
    }
}

#[test]
fn a8_3_flag_defaults_off_parses_and_allocates_bit_512() {
    assert!(!Config::default().tint_balance);
    assert!(cfg_of(&["--tint-balance"]).tint_balance);
    assert_eq!(FLAG_HI_TINT_BALANCE, 512);
}

#[test]
fn a8_3_override_default_is_dark() {
    let src = render::shader_source();
    assert!(src.contains("override SPEC_TINT_BALANCE: bool = false;"));

    assert!(
        src.contains("let plains = select(TINT_PLAINS, TINT_PLAINS_BALANCED, SPEC_TINT_BALANCE);"),
        "the swap has to be the compile-time select -- a runtime branch is a register \
         batch 67 already priced"
    );
}

#[test]
fn a8_3_the_balanced_plains_is_not_the_identity() {
    let src = render::shader_source();
    assert!(src.contains("TINT_PLAINS_BALANCED: vec3<f32> = vec3<f32>(0.935, 1.067, 0.892)"));
    assert!(!src.contains("TINT_PLAINS_BALANCED: vec3<f32> = vec3<f32>(1.000, 1.000, 1.000)"));
}

#[test]
fn a8_3_the_masks_land_on_exactly_two_layers() {
    use voxelcraft::block::tex;
    use voxelcraft::textures::{self, TEX_SIZE};
    let off = textures::build_atlas(0.0, false);
    let on = textures::build_atlas(0.0, true);
    assert_eq!(off.len(), on.len());
    let texels = TEX_SIZE as usize * TEX_SIZE as usize;
    let layer_px = |atlas: &[u8], layer: u32, i: usize| -> [u8; 4] {
        let o = (layer as usize * texels + i) * 4;
        [atlas[o], atlas[o + 1], atlas[o + 2], atlas[o + 3]]
    };
    for layer in 0..tex::COUNT {
        let mut diffs = 0usize;
        let mut alphas_now = 0usize;
        let mut colour_moved = 0usize;
        for i in 0..texels {
            let a = layer_px(&off, layer, i);
            let b = layer_px(&on, layer, i);
            if a != b {
                diffs += 1;
                if a[..3] != b[..3] {
                    colour_moved += 1;
                } else if a[3] == 0 && b[3] == 255 {
                    alphas_now += 1;
                }
            }
        }
        match layer {
            tex::SAND => {
                assert_eq!(diffs, texels, "sand: every texel masked, none recoloured");
                assert_eq!(colour_moved, 0);
            }
            tex::LICHEN => {
                assert!(
                    alphas_now > texels / 2,
                    "lichen: the moss masked, the stone left bare (only {alphas_now}/{texels})"
                );
                assert_eq!(
                    colour_moved, 0,
                    "lichen: content changed colour, not just alpha"
                );
            }
            _ => {
                assert_eq!(diffs, 0, "layer {layer} moved -- masks are content")
            }
        }
    }
}

#[test]
fn a8_4_flag_defaults_off_and_parses() {
    assert!(!Config::default().meadow_side);
    assert!(cfg_of(&["--meadow-side"]).meadow_side);
}

#[test]
fn a8_4_the_two_cubes_differ_in_one_face() {
    use voxelcraft::block::{self, tex};
    let old = block::def(block::MEADOW);
    let new = block::def(block::MEADOW_SIDE);
    assert_eq!(block::MEADOW_SIDE, 23, "appended, never inserted");

    assert_eq!(block::REEDS, 25, "101e's grass pair appended after the meadow pair");
    assert_eq!(block::REEDS as usize, block::BLOCK_COUNT - 1);

    for f in [0usize, 1, 4, 5] {
        assert_eq!(
            new.faces[f],
            tex::MEADOW_SIDE,
            "side face {f} is the new slice"
        );
        assert_eq!(
            old.faces[f],
            tex::GRASS_SIDE,
            "side face {f}: the unarmed cube did not move"
        );
    }
    assert_eq!(new.faces[2], tex::DIRT, "bottom stays dirt");
    assert_eq!(new.faces[3], tex::MEADOW_TOP, "top stays the carpet");
    assert_eq!(new.faces[2], old.faces[2]);
    assert_eq!(new.faces[3], old.faces[3]);
    assert_eq!(
        block::ALBEDO[block::MEADOW_SIDE as usize],
        block::ALBEDO[block::MEADOW as usize],
        "the top face is the same layer, so the bounce table says the same thing"
    );
}

#[test]
fn a8_4_the_slice_shapes_a_side() {
    use voxelcraft::block::tex;
    use voxelcraft::textures::{self, TEX_SIZE};
    let atlas = textures::build_atlas(0.0, false);
    let texels = TEX_SIZE as usize * TEX_SIZE as usize;
    let px_at = |layer: u32, x: usize, y: usize| -> [u8; 4] {
        let o = (layer as usize * texels + y * TEX_SIZE as usize + x) * 4;
        [atlas[o], atlas[o + 1], atlas[o + 2], atlas[o + 3]]
    };

    let in_range = |px: [u8; 4], c: [f32; 3], lo: f32, hi: f32, shade: f32| -> bool {
        c.iter().enumerate().all(|(ch, &base)| {
            let b = px[ch] as f32;
            b >= (base * lo * shade * 255.0).floor() - 1.0
                && b <= (base * hi * shade * 255.0).ceil() + 1.0
        })
    };
    let grass = [0.36f32, 0.62, 0.24];
    let dirt = [0.53f32, 0.38, 0.26];
    let shade = 0.59f32;
    for x in 0..TEX_SIZE as usize {

        for y in 0..2usize {
            let p = px_at(tex::MEADOW_SIDE, x, y);
            assert_eq!(p[3], 255, "fringe ({x},{y}): the carpet must tint");
            assert!(
                in_range(p, grass, 0.78, 1.10, shade),
                "fringe ({x},{y}) = {p:?}: outside the MEADOW_SHADE-scaled grass band --                  either the scalar or the base colour moved, and the stand-in no longer                  tracks the carpet"
            );
        }

        for y in (TEX_SIZE as usize - 2)..TEX_SIZE as usize {
            let p = px_at(tex::MEADOW_SIDE, x, y);
            assert_eq!(p[3], 0, "dirt ({x},{y}): dirt does not tint (grass-side rule)");
            assert!(
                in_range(p, dirt, 0.75, 1.10, shade),
                "dirt ({x},{y}) = {p:?}: outside the MEADOW_SHADE-scaled dirt band"
            );
        }
    }
}

#[test]
fn a8_4_armed_world_swaps_the_id_and_nothing_else() {
    use glam::IVec3;
    use voxelcraft::block::{AIR, MEADOW, MEADOW_SIDE};
    use voxelcraft::voxel::{ChunkKey, VOL};
    use voxelcraft::worldgen::WorldGen;
    let off = WorldGen::new(91);
    let mut on = WorldGen::new(91);
    on.meadow_side = true;
    assert!(!off.meadow_side);
    let chunk_of =
        |gen: &WorldGen, lod: u8, x: i32, y: i32, z: i32| -> Vec<voxelcraft::block::BlockId> {
            let mut dense = vec![AIR; VOL];
            let mut heights = Box::new([0i32; 64 * 64]);
            gen.generate(
                ChunkKey::new(lod, IVec3::new(x, y, z)),
                &mut dense,
                &mut heights,
            );
            dense
        };
    let mut swaps = 0usize;
    let mut mismatches = 0usize;
    for (lod, cx, cy, cz) in [(1, 2, 1, 2), (1, -3, 1, 5), (2, 1, 1, 1), (2, -2, 1, -2)] {
        let a = chunk_of(&off, lod, cx, cy, cz);
        let b = chunk_of(&on, lod, cx, cy, cz);
        for i in 0..VOL {
            if a[i] != b[i] {
                if a[i] == MEADOW && b[i] == MEADOW_SIDE {
                    swaps += 1;
                } else {
                    mismatches += 1;
                }
            }
        }
        assert!(
            !b.contains(&MEADOW),
            "armed chunk ({lod},{cx},{cy},{cz}) kept a legacy cube"
        );
    }
    assert!(
        swaps > 0,
        "no meadow across the sweep -- the test never fired"
    );
    assert_eq!(mismatches, 0, "the arm moved a voxel it did not name");
}
