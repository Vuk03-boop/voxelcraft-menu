//! The second specialization word (FLAG_HI / SPEC_HI_MASK) asserted as *properties*, not
//! counts. The standing lesson the user's third round of census failures wrote down: a test
//! that pins "the mask has exactly N bits" means "every bit is distinct and in the mask" and
//! pays for the misspelling with a re-pinned integer in a different file every batch. **The
//! enumeration lives here, once** -- adding a FLAG_HI bit means adding one line to `ALL_HI`
//! below and nothing anywhere else; the four assertions all hold as properties of the set.

use voxelcraft::render::{
    FLAG_HI_LEGACY_LIGHTING, FLAG_HI_LEGACY_WATER, FLAG_HI_SOFT_HQ, FLAG_HI_DEBUG_A, FLAG_HI_DEBUG_B, FLAG_HI_DEBUG_C, FLAG_HI_SUN_NARROW, FLAG_HI_SUN_WIDE,
    FLAG_HI_CAUSTICS, FLAG_HI_EMITTER_WORLD, FLAG_HI_FULL_MARCH, FLAG_HI_GLASS_REFLECT,
    FLAG_HI_SHORE_FOAM, FLAG_HI_SHORE_WET, FLAG_HI_SKY_SPECULAR, FLAG_HI_SNELL_BEND,
    FLAG_HI_CANOPY_RELIEF, FLAG_HI_FOLIAGE_RICH, FLAG_HI_SKY_COOL, FLAG_HI_SKY_LOOK,
    FLAG_HI_TINT_BALANCE, FLAG_HI_WATER_LOOK, FLAG_HI_WATER_SEC, FLAG_HI_WIND_SWAY,
    FLAG_HI_SOFT_SHADOWS, FLAG_HI_COMPACT_SHADE_HIT, FLAG_HI_ISOLATE_GLASS, FLAG_HI_SHADOW_PASS, SPEC_HI_MASK,
};

/// Every FLAG_HI bit in the tree. Kept next to the property assertions that read it so the
/// line that has to grow is the line you are already looking at.
const ALL_HI: &[(u32, &str)] = &[
    (FLAG_HI_LEGACY_LIGHTING, "FLAG_HI_LEGACY_LIGHTING"),
    (FLAG_HI_LEGACY_WATER, "FLAG_HI_LEGACY_WATER"),
    (FLAG_HI_SOFT_HQ, "FLAG_HI_SOFT_HQ"),
    (FLAG_HI_DEBUG_A, "FLAG_HI_DEBUG_A"),
    (FLAG_HI_DEBUG_B, "FLAG_HI_DEBUG_B"),
    (FLAG_HI_DEBUG_C, "FLAG_HI_DEBUG_C"),
    (FLAG_HI_SUN_NARROW, "FLAG_HI_SUN_NARROW"),
    (FLAG_HI_SUN_WIDE, "FLAG_HI_SUN_WIDE"),

    (FLAG_HI_SKY_SPECULAR, "FLAG_HI_SKY_SPECULAR"),
    (FLAG_HI_FULL_MARCH, "FLAG_HI_FULL_MARCH"),
    (FLAG_HI_EMITTER_WORLD, "FLAG_HI_EMITTER_WORLD"),
    (FLAG_HI_SHORE_WET, "FLAG_HI_SHORE_WET"),
    (FLAG_HI_SHORE_FOAM, "FLAG_HI_SHORE_FOAM"),
    (FLAG_HI_WATER_SEC, "FLAG_HI_WATER_SEC"),
    (FLAG_HI_CAUSTICS, "FLAG_HI_CAUSTICS"),
    (FLAG_HI_GLASS_REFLECT, "FLAG_HI_GLASS_REFLECT"),
    // Batch 91 grew the word here and nowhere else: one line in this enumeration.
    (FLAG_HI_SNELL_BEND, "FLAG_HI_SNELL_BEND"),
    // Batch 92, same sentence: the word's first constant-*swap* arm.
    (FLAG_HI_TINT_BALANCE, "FLAG_HI_TINT_BALANCE"),
    // Batch 95: one bit for four knobs, because the uniform-guard class failed the
    // 21-vantage control -- the fold is what the control's bit-exactness rides on.
    (FLAG_HI_SKY_LOOK, "FLAG_HI_SKY_LOOK"),
    // Batch 97: the look-gap five, four bits behind overrides and one blit selector
    // value (cine, no bit -- the A10 blit class needs no pipeline). The word's whole 97
    // allocation arrived with the first arm that uses it (water); the foliage and
    // canopy rows name bodies whose gates land with their own commits.
    (FLAG_HI_SKY_COOL, "FLAG_HI_SKY_COOL"),
    (FLAG_HI_WATER_LOOK, "FLAG_HI_WATER_LOOK"),
    (FLAG_HI_FOLIAGE_RICH, "FLAG_HI_FOLIAGE_RICH"),
    (FLAG_HI_CANOPY_RELIEF, "FLAG_HI_CANOPY_RELIEF"),
    (FLAG_HI_WIND_SWAY, "FLAG_HI_WIND_SWAY"),
    // Batch 102a: the sun-disc penumbra -- the first arm whose *cost* is its content
    // (rays, not ALU), which is exactly why the fold is its only revert story.
    (FLAG_HI_SOFT_SHADOWS, "FLAG_HI_SOFT_SHADOWS"),
    (FLAG_HI_COMPACT_SHADE_HIT, "FLAG_HI_COMPACT_SHADE_HIT"),
    (FLAG_HI_ISOLATE_GLASS, "FLAG_HI_ISOLATE_GLASS"),
    (FLAG_HI_SHADOW_PASS, "FLAG_HI_SHADOW_PASS"),
];

/// Every constant selects exactly one bit -- "a bit" being the property, not which one: the
/// numbering itself is allocation detail that only the definitions are allowed to fix.
#[test]
fn every_flag_hi_is_one_power_two() {
    for &(bit, name) in ALL_HI {
        assert!(bit.is_power_of_two(), "{name} = {bit} is not one bit");
    }
}

/// Every one of them is a distinct channel: two controls keying one pipeline is the failure
/// `ensure_spec` cannot see, and the pairwise statement is what a count was standing in for.
#[test]
fn every_flag_hi_pair_is_distinct() {
    for (i, &(a, na)) in ALL_HI.iter().enumerate() {
        for &(b, nb) in &ALL_HI[i + 1..] {
            assert_eq!(a & b, 0, "{na} ({a}) overlaps {nb} ({b})");
        }
    }
}

/// Every named bit keys a pipeline: if a flag is not in the mask, the build carrying it off
/// and the build carrying it on produce one compiled module and the revert is a fiction.
#[test]
fn every_flag_hi_is_in_the_mask() {
    for &(bit, name) in ALL_HI {
        assert_eq!(
            SPEC_HI_MASK & bit,
            bit,
            "{name} ({bit}) is not in SPEC_HI_MASK"
        );
    }
}

/// And the mask holds nothing anonymous -- every bit it turns is a bit with a name, so the
/// pipeline key space is fully accounted for by `ALL_HI`. This is the enumeration's only
/// obligation beyond the properties: one line per new constant, here, or this fails with the
/// orphan bit spelled in the message.
#[test]
fn the_mask_is_exactly_the_named_bits() {
    let mut u = 0u32;
    for &(bit, _) in ALL_HI {
        u |= bit;
    }
    assert_eq!(
        SPEC_HI_MASK,
        u,
        "mask bits not in ALL_HI: {:#x}; ALL_HI bits not in mask: {:#x}",
        SPEC_HI_MASK & !u,
        u & !SPEC_HI_MASK
    );
}

