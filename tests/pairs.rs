//! Batch 53: the two limits on the `(tile, chunk)` pair word, which are not the same limit.
//!
//! `tile_select` appends `(tile_id << 13) | chunk_id` into a list. Two things can overflow:
//!
//! - **the list**, which has been warned about since batch 22 and loses geometry visibly
//!   enough that `harness::render` refuses to score the capture;
//! - **the word**, which had no guard at all until this batch and does not lose geometry --
//!   it renders *another tile's* geometry, in a frame that is lit, shaded, temporally stable
//!   and wrong.
//!
//! Roadmap P4 carried the first as "reachable at 8K". Measured at `default`, the pair list
//! asked for **1,529,818 at 5120x2880** against a fixed cap of 1,048,576 -- 46% over, and
//! truncating -- and **80.5% of the cap at plain 3840x2160**, where one `--scale 2` or one
//! busier camera would have gone over. It was reachable two rungs below where the roadmap
//! said, and the entry is corrected rather than quietly fixed.
//!
//! These are arithmetic claims about constants, so they are a test rather than a sweep: the
//! resolutions that reach either limit take a 1.6 GB allocation and a minute each to render,
//! which is exactly the kind of check that stops being run.

use voxelcraft::render::{self, MAX_CHUNKS, MAX_PAIRS, MAX_TILES, TILE_ID_BITS};

/// Tiles on a screen, the way `Renderer::resize` counts them.
fn tiles(w: u32, h: u32) -> (u32, u32) {
    (w.div_ceil(8), h.div_ceil(8))
}

fn tile_count(w: u32, h: u32) -> u64 {
    let t = tiles(w, h);
    u64::from(t.0) * u64::from(t.1)
}

/// The two fields of the pair word account for all 32 bits and neither may be widened alone.
#[test]
fn the_pair_word_is_a_tile_field_and_a_chunk_field() {
    assert_eq!(
        TILE_ID_BITS + 13,
        32,
        "the pair word is `(tile_id << 13) | chunk_id` in a u32, so the two fields have to \
         add to 32. Widening either means narrowing the other, and the chunk field's 13 bits \
         are what cap the frame list at MAX_CHUNKS."
    );
    assert_eq!(MAX_TILES, 1 << TILE_ID_BITS);
    assert_eq!(
        MAX_CHUNKS,
        (1usize << 13) - 1,
        "MAX_CHUNKS is the 13-bit chunk field, all ones, and the shader masks the pair \n         word with 0x1FFF to read it back. If these ever disagree a chunk id wraps \n         into the tile index and the pair names a tile nobody selected"
    );
}

/// **The capacity at every resolution this project has ever measured is unchanged.**
///
/// This is the test that lets batch 53 touch the pair buffer at all. `PERF.md`'s frame map is
/// 1920x1080 and the whole fixture is 1280x720; if `pairs_for` returned anything but the old
/// fixed `MAX_PAIRS` at either, every number in the ledger would be describing a different
/// allocation from the one that produced it, and the `bitexact` sweep that passed this batch
/// would have been a sweep of two different renderers.
#[test]
fn the_measured_resolutions_keep_exactly_the_capacity_they_had() {
    for (w, h) in [(1280, 720), (1920, 1080), (2560, 1440)] {
        assert_eq!(
            render::pairs_for(tiles(w, h)),
            MAX_PAIRS,
            "{w}x{h} should sit on the MAX_PAIRS floor, so it allocates what it always did"
        );
    }
}

/// Past the floor the budget covers what was actually measured, with room.
///
/// The demands are at `default`, which is not the worst camera in the set -- it is the one the
/// numbers were taken at. The margin is what `PAIRS_PER_TILE` is for, and
/// `Renderer::warn_if_truncated` is what covers the case no budget can.
#[test]
fn the_scaled_budget_covers_the_measured_demand() {
    // (width, height, select + deferred measured at `default` before the budget was raised)
    for (w, h, demand) in [(3840u32, 2160u32, 1_313_456u64), (5120, 2880, 2_325_513)] {
        let cap = u64::from(render::pairs_for(tiles(w, h)));
        assert!(
            cap > demand,
            "{w}x{h} budgets {cap} pairs against a measured demand of {demand}; the fixed \
             1 << 20 truncated here, which is the defect this replaces"
        );
    }
}

/// **The narrower limit, and the one that renders a plausible wrong frame.**
///
/// 7680x4320 is 518,400 tiles against 524,288 -- 98.9%, one step of width from wrapping. That
/// margin is the reason this test exists rather than a comment: the next person to reach for a
/// bigger `--screenshot`, or for `--scale 2` on a 4K one, is one step away and nothing in the
/// frame would tell them.
#[test]
fn eight_k_fits_the_tile_field_and_the_next_step_up_does_not() {
    assert!(
        tile_count(7680, 4320) <= u64::from(MAX_TILES),
        "7680x4320 is {} tiles against MAX_TILES {MAX_TILES}; 8K is a size this project's own \
         roadmap names, so it has to fit",
        tile_count(7680, 4320)
    );
    assert!(
        tile_count(8192, 4320) > u64::from(MAX_TILES),
        "8192x4320 is {} tiles, which should be over MAX_TILES {MAX_TILES}. If this ever \
         passes, the field was widened and `warn_if_oversized`'s threshold moved with it -- \
         check that the chunk field did not pay for it.",
        tile_count(8192, 4320)
    );
}

/// `--scale` multiplies the internal resolution, so it reaches both limits ahead of the window.
///
/// Recorded as a test because it is the argument most likely to reach them and the least
/// likely to be suspected: the user asks for a 4K screenshot and gets an 8K trace.
#[test]
fn scale_two_at_four_k_is_the_eight_k_case() {
    assert_eq!(tile_count(3840 * 2, 2160 * 2), tile_count(7680, 4320));
    assert!(
        u64::from(render::pairs_for(tiles(7680, 4320))) > 2_325_513,
        "a --scale 2 capture at 4K traces at 8K and has to budget for it"
    );
}



