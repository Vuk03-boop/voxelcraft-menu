use voxelcraft::block;
use voxelcraft::light::{self, LightData};
use voxelcraft::voxel::tree::{cell_bit, dense_index};
use voxelcraft::voxel::VOL;

const UNIFORM_BIT: u32 = 1 << 31;

fn probe(d: &LightData, x: usize, y: usize, z: usize) -> u16 {
    if let Some(u) = d.uniform {
        return u;
    }
    let cell = (x >> 2) | ((y >> 2) << 4) | ((z >> 2) << 8);
    let entry = d.cells[cell];
    if entry & UNIFORM_BIT != 0 {
        return (entry & 0xFFFF) as u16;
    }
    let vb = cell_bit((x & 3) as u32, (y & 3) as u32, (z & 3) as u32) as usize;
    ((d.bricks[entry as usize][vb >> 1] >> ((vb & 1) * 16)) & 0xFFFF) as u16
}

fn room(lamps: &[(usize, usize, usize, block::BlockId)]) -> LightData {
    let mut dense = vec![block::STONE; VOL];
    for z in 21..42usize {
        for y in 21..30usize {
            for x in 21..42usize {
                dense[dense_index(x, y, z)] = block::AIR;
            }
        }
    }
    for &(x, y, z, id) in lamps {
        dense[dense_index(x, y, z)] = id;
    }
    light::compute(&dense, &vec![false; 64 * 64])
}

#[test]
fn the_scalar_control_reproduces_the_old_flood() {
    assert_eq!(
        block::def(block::GLOWSTONE).light[0],
        light::MAX_LEVEL,
        "glowstone's red channel is the pre-59 scalar level; re-authoring it below MAX_LEVEL \
         makes --no-light-rgb a rescale of the old frame rather than a revert of it"
    );
    let data = room(&[(31, 25, 31, block::GLOWSTONE)]);
    let mut lit = 0;
    for z in 21..42usize {
        for y in 21..30usize {
            for x in 21..42usize {
                let c = probe(&data, x, y, z);
                let ch = light::block_of(c);
                assert_eq!(
                    light::block_level(c),
                    ch[0],
                    "at ({x},{y},{z}) the three channels are {ch:?}: red is the pre-59 flood \
                     and must be the largest of them everywhere it reached"
                );
                if ch[0] > 0 {
                    lit += 1;
                }
            }
        }
    }

    assert!(
        lit > 500,
        "only {lit} cells carry block light; the flood did not fill the room and the loop \
         above asserted nothing"
    );
}

#[test]
fn two_lamps_of_different_colours_mix() {
    let amber = block::def(block::AMBER_LAMP).light;
    let azure = block::def(block::AZURE_LAMP).light;

    assert!(amber[0] > azure[0] && azure[2] > amber[2], "{amber:?} {azure:?}");

    let data = room(&[
        (22, 25, 31, block::AMBER_LAMP),
        (40, 25, 31, block::AZURE_LAMP),
    ]);
    let mid = light::block_of(probe(&data, 31, 25, 31));

    let want_r = amber[0].saturating_sub(9);
    let want_b = azure[2].saturating_sub(9);
    assert!(
        want_r > 0 && want_b > 0,
        "the lamps do not reach the midpoint at all ({amber:?}, {azure:?}); this test is \
         measuring a dark cell"
    );
    assert_eq!(
        mid[0], want_r,
        "the midpoint's red is {mid:?}, and red at that distance is the amber lamp's alone"
    );
    assert_eq!(
        mid[2], want_b,
        "the midpoint's blue is {mid:?}, and blue at that distance is the azure lamp's alone"
    );
}

#[test]
fn every_emitter_may_be_placed() {
    let mut emitters = 0;
    for id in 0..block::BLOCK_COUNT {
        let def = block::def(id as block::BlockId);
        if def.light == [0, 0, 0] {
            continue;
        }
        emitters += 1;
        assert!(
            block::bar_slot_refusal(id as block::BlockId).is_none(),
            "{} emits {:?} and may not be placed: {}",
            def.name,
            def.light,
            block::bar_slot_refusal(id as block::BlockId).unwrap()
        );
    }

    assert_eq!(
        emitters, 4,
        "the table holds {emitters} emitters; batch 59 shipped four, and a coloured flood with \
         one white emitter in the world is the mistake roadmap R4 exists to have avoided"
    );
}

#[test]
fn pack_round_trips() {
    for sky in 0..=light::MAX_LEVEL {
        for &blk in &[[0u8, 0, 0], [15, 13, 9], [3, 9, 15], [1, 0, 15]] {
            let c = light::pack(sky, blk);
            assert_eq!(light::sky_of(c), sky, "sky of {c:#06x}");
            assert_eq!(light::block_of(c), blk, "block of {c:#06x}");
        }
    }

    assert_eq!(light::pack(light::MAX_LEVEL, [0, 0, 0]), 0xF000);
}

#[test]
fn light_cell_is_two_bytes_wide() {

    assert_eq!(light::BRICK_WORDS, 32);
    let src = voxelcraft::render::shader_source();
    for want in [

        "bricks[entry + (vb >> 1u)]",
        "(w >> ((vb & 1u) * 16u)) & 0xFFFFu",

        "(bricks[brick + (vb >> 1u)] >> ((vb & 1u) * 16u)) & 0xFFFFu",

        "return entry & 0xFFFFu;",
    ] {
        assert!(
            src.contains(want),
            "common.wgsl no longer decodes a light cell as two bytes: {want:?} is gone, and a \
             cell read at the old byte stride reads a neighbouring cell rather than failing"
        );
    }

    for gone in ["(vb >> 2u)] >> ((vb & 3u) * 8u)", "return entry & 0xFFu;"] {
        assert!(
            !src.contains(gone),
            "common.wgsl still holds the pre-59 byte decode {gone:?} somewhere"
        );
    }
}

#[test]
fn glowstone_is_the_retired_block_tint() {

    const BLOCK_TINT: [f32; 3] = [1.0, 0.65, 0.32];
    let curve = |l: u8| {
        let x = l as f32 / 15.0;
        x * x * (0.6 + 0.4 * x)
    };
    let got = block::def(block::GLOWSTONE).light;
    for (c, &want) in BLOCK_TINT.iter().enumerate() {
        let err = (curve(got[c]) - want).abs();
        for l in 0..=light::MAX_LEVEL {
            assert!(
                (curve(l) - want).abs() >= err - 1e-6,
                "glowstone channel {c} is level {} ({:.4} against BLOCK_TINT's {want}), but \
                 level {l} is {:.4} and lands nearer -- the row is meant to be the closest \
                 4-bit spelling of the constant it retired",
                got[c],
                curve(got[c]),
                curve(l)
            );
        }
    }

    assert!((curve(got[0]) - 1.0).abs() < 1e-6, "red is exact at level 15");
    assert!(
        (curve(got[1]) - BLOCK_TINT[1]).abs() > 0.05,
        "green landing inside 0.05 of the target means the channel is no longer 4 bits, and \
         `docs/shading.md`'s account of what this row costs is stale"
    );
}
