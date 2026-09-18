//! Coloured block light: the widened cell, the mixing it buys, and the identity the control
//! rests on. Batch 59, roadmap R4.
//!
//! **The batch's central claim is a bit-exactness claim about a build that no longer exists**,
//! and that is what this file is for. `--no-light-rgb` is supposed to reproduce the pre-59
//! frame *to the bit* at every world that predates the batch, and the sweep can only say that
//! it does at the eighteen cameras the fixture owns. What makes it true everywhere is an
//! arithmetic identity -- every channel of an emitter decrements by one per flood step, so
//! `max(r, g, b)` at any cell is the level the single pre-59 flood held there -- and an
//! identity is a thing to assert rather than to sample.
//!
//! The other half is the inverse and is just as easy to lose. A control that is bit-exact and
//! a feature that was never wired up produce the same sweep, which `lessons.md` records and
//! `tests/probe.rs` opens with; here the positive is that two lamps of different colours
//! **mix**, which is the one thing no per-block tint applied at read time can do however the
//! tint is chosen, and it is asserted rather than left to a picture.

use voxelcraft::block;
use voxelcraft::light::{self, LightData};
use voxelcraft::voxel::tree::{cell_bit, dense_index};
use voxelcraft::voxel::VOL;

const UNIFORM_BIT: u32 = 1 << 31;

/// Decode one voxel's packed light cell out of a [`LightData`], by coordinate.
///
/// A transliteration of `pack_bricks` run backwards, and a third copy of one that already
/// exists in `tests/water.rs` and `tests/glass.rs`. `pitfalls.md` records clippy being refused
/// on exactly these, because what they are worth is reading the same as the original -- and
/// **all three had to change in batch 59**, which is the argument for the census test below
/// rather than for folding them into one helper: a shared helper would have been updated once
/// and told nobody that two other files had also been reading a byte.
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

/// A sealed room in the middle of a chunk, with whatever is passed set into one wall.
///
/// Sealed on purpose: `open` is all `false`, so no sky reaches any cell and every level this
/// file reads is block light alone. A test that let daylight in would still pass and would be
/// measuring the pour.
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

/// `max(r, g, b)` is the level the single pre-59 flood held, at every cell it reached.
///
/// **This is what `--no-light-rgb` rests on and it is not a property of the shader.** The
/// control reads `light_block_level`, which is this `max`; if it ever stopped agreeing with the
/// old scalar flood the control would render something close to the pre-59 frame rather than
/// equal to it, and a sweep whose pass condition is *0 pixels differing* would report the
/// difference without being able to say which side was wrong.
///
/// The identity holds because every channel decrements by one per step from its own source
/// level, so at distance `d` the three are `(r0 - d, g0 - d, b0 - d)` clamped at zero and the
/// largest is `max(r0, g0, b0) - d`. Glowstone's row is authored with red at
/// [`light::MAX_LEVEL`], which is exactly what it emitted before the batch -- so for a world
/// holding only glowstone, red *is* the old flood and the assert is `max == r`.
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
    // A room the flood never entered would pass every assertion above it, which is the failure
    // mode that looks like success -- and this file's own `room` helper seals the ceiling, so
    // "no light anywhere" is exactly what a mistake here produces.
    assert!(
        lit > 500,
        "only {lit} cells carry block light; the flood did not fill the room and the loop \
         above asserted nothing"
    );
}

/// Two lamps of different colours mix, and the cell between them carries both.
///
/// **This is the claim a scalar channel cannot make, and it is the whole reason the cell had to
/// widen rather than the block table.** A per-block tint applied at read time has one number to
/// work from at each cell and no record of which block put it there, so a wall between an amber
/// lamp and an azure one would take one hue or the other. Three independent floods carry both,
/// and the assertion is that the midpoint has more red than the azure lamp emits *and* more
/// blue than the amber one does.
#[test]
fn two_lamps_of_different_colours_mix() {
    let amber = block::def(block::AMBER_LAMP).light;
    let azure = block::def(block::AZURE_LAMP).light;
    // The premise of the test, stated rather than assumed: these two disagree about which
    // channel is bright. Authoring them to the same hue would leave every assertion below
    // passing and nothing demonstrated.
    assert!(amber[0] > azure[0] && azure[2] > amber[2], "{amber:?} {azure:?}");

    let data = room(&[
        (22, 25, 31, block::AMBER_LAMP),
        (40, 25, 31, block::AZURE_LAMP),
    ]);
    let mid = light::block_of(probe(&data, 31, 25, 31));
    // Nine blocks from each lamp, so each contributes its own source level minus nine.
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

/// Every emitter in the table may sit in a hotbar slot.
///
/// Roadmap R4 asks for "the hotbar rule at `block::bar_slot_refusal` applied to each", and this
/// is that rule applied rather than restated. It is a **census** and not three assertions:
/// `BLOCK_COUNT` is what it iterates, so a lamp appended by a later batch is covered on the day
/// it is appended rather than on the day somebody remembers this file.
///
/// The rule matters here for `FLAG_LIGHT_RGB`'s own reason. `scene::flags_from` sets that bit
/// unconditionally -- the shipping build must be able to read a channel the player is about to
/// fill -- and a placeable emitter is exactly how a world with no lamp in it acquires one.
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
    // Batch 55's finding was that this count was **one**, which is why R4 is content as much as
    // it is code. The number is asserted rather than described so that a batch which deletes
    // the lamps has to say so here.
    assert_eq!(
        emitters, 4,
        "the table holds {emitters} emitters; batch 59 shipped four, and a coloured flood with \
         one white emitter in the world is the mistake roadmap R4 exists to have avoided"
    );
}

/// The packed cell round-trips, and sky is the top nibble.
#[test]
fn pack_round_trips() {
    for sky in 0..=light::MAX_LEVEL {
        for &blk in &[[0u8, 0, 0], [15, 13, 9], [3, 9, 15], [1, 0, 15]] {
            let c = light::pack(sky, blk);
            assert_eq!(light::sky_of(c), sky, "sky of {c:#06x}");
            assert_eq!(light::block_of(c), blk, "block of {c:#06x}");
        }
    }
    // The one literal the shader carries rather than derives: `LIGHT_OPEN_SKY` is what
    // `world_sample` returns for a chunk that is not resident, and it has to be full sky with
    // no block light or a missing neighbour would light a face.
    assert_eq!(light::pack(light::MAX_LEVEL, [0, 0, 0]), 0xF000);
}

/// The brick layout is mirrored in `common.wgsl`, and this is the only thing holding the copies
/// together.
///
/// A shader cannot see this crate, so `light_at` spells out the same shift by hand. **A wrong
/// shift here does not produce garbage**, which is why this is a census and not a bug report:
/// it reads the neighbouring cell's nibbles, so every surface in the world is lit by a plausible
/// level belonging to somewhere else. That is `block_faces`' stride of eight one subsystem over,
/// and it went unnoticed for eight batches.
#[test]
fn light_cell_is_two_bytes_wide() {
    // 64 cells to a 4^3 brick, two cells to a 32-bit word.
    assert_eq!(light::BRICK_WORDS, 32);
    let src = voxelcraft::render::shader_source();
    for want in [
        // The brick word and the half of it this cell lives in.
        "bricks[entry + (vb >> 1u)]",
        "(w >> ((vb & 1u) * 16u)) & 0xFFFFu",
        // The same decode again inside `gather_face`, which walks the nine cells of a face by
        // hand rather than calling `light_at` nine times.
        "(bricks[brick + (vb >> 1u)] >> ((vb & 1u) * 16u)) & 0xFFFFu",
        // The inline uniform entry, which is the cell itself and not an index.
        "return entry & 0xFFFFu;",
    ] {
        assert!(
            src.contains(want),
            "common.wgsl no longer decodes a light cell as two bytes: {want:?} is gone, and a \
             cell read at the old byte stride reads a neighbouring cell rather than failing"
        );
    }
    // The byte-wide spellings must be gone, not merely outnumbered: leaving one behind is how
    // a mixed layout ships, with most of the world lit correctly.
    for gone in ["(vb >> 2u)] >> ((vb & 3u) * 8u)", "return entry & 0xFFu;"] {
        assert!(
            !src.contains(gone),
            "common.wgsl still holds the pre-59 byte decode {gone:?} somewhere"
        );
    }
}

/// Glowstone's row is the closest 4-bit spelling of the constant batch 59 retired.
///
/// **`BLOCK_TINT` was the colour of all block light and glowstone's row is where it went**, so
/// the row is derived and not chosen -- and a derivation is a thing to check rather than to
/// describe. `light_curve` is not linear, so authoring a hue means authoring the levels whose
/// *curved* values carry it, and the test is that no other level in 0..=15 lands nearer.
///
/// Green is the interesting channel and the reason this is worth a test at all: 12 and 13
/// bracket the target almost symmetrically, 9.4% either side, so the row records a tie-break
/// rather than a fit and the next person to look at it should be able to see that it is tight
/// rather than sloppy.
#[test]
fn glowstone_is_the_retired_block_tint() {
    // `resolve.wgsl`'s `BLOCK_TINT`, which is the pre-59 colour of every emitter in the world.
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
    // The tie-break, asserted so it cannot be lost: red is exact at 15, and green is 9.4% away
    // whichever side it is taken from. If a later batch widens the channel, this is the test
    // that should start failing rather than the one that should be deleted.
    assert!((curve(got[0]) - 1.0).abs() < 1e-6, "red is exact at level 15");
    assert!(
        (curve(got[1]) - BLOCK_TINT[1]).abs() > 0.05,
        "green landing inside 0.05 of the target means the channel is no longer 4 bits, and \
         `docs/shading.md`'s account of what this row costs is stale"
    );
}



