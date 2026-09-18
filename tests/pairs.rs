use voxelcraft::render::{self, MAX_CHUNKS, MAX_PAIRS, MAX_TILES, TILE_ID_BITS};

fn tiles(w: u32, h: u32) -> (u32, u32) {
    (w.div_ceil(8), h.div_ceil(8))
}

fn tile_count(w: u32, h: u32) -> u64 {
    let t = tiles(w, h);
    u64::from(t.0) * u64::from(t.1)
}

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

#[test]
fn the_scaled_budget_covers_the_measured_demand() {

    for (w, h, demand) in [(3840u32, 2160u32, 1_313_456u64), (5120, 2880, 2_325_513)] {
        let cap = u64::from(render::pairs_for(tiles(w, h)));
        assert!(
            cap > demand,
            "{w}x{h} budgets {cap} pairs against a measured demand of {demand}; the fixed \
             1 << 20 truncated here, which is the defect this replaces"
        );
    }
}

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

#[test]
fn scale_two_at_four_k_is_the_eight_k_case() {
    assert_eq!(tile_count(3840 * 2, 2160 * 2), tile_count(7680, 4320));
    assert!(
        u64::from(render::pairs_for(tiles(7680, 4320))) > 2_325_513,
        "a --scale 2 capture at 4K traces at 8K and has to budget for it"
    );
}
