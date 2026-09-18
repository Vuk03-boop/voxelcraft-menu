use voxelcraft::render::{
    FLAG_HI_LEGACY_LIGHTING, FLAG_HI_LEGACY_WATER, FLAG_HI_SOFT_HQ, FLAG_HI_DEBUG_A, FLAG_HI_DEBUG_B, FLAG_HI_DEBUG_C, FLAG_HI_SUN_NARROW, FLAG_HI_SUN_WIDE,
    FLAG_HI_CAUSTICS, FLAG_HI_EMITTER_WORLD, FLAG_HI_FULL_MARCH, FLAG_HI_GLASS_REFLECT,
    FLAG_HI_SHORE_FOAM, FLAG_HI_SHORE_WET, FLAG_HI_SKY_SPECULAR, FLAG_HI_SNELL_BEND,
    FLAG_HI_CANOPY_RELIEF, FLAG_HI_FOLIAGE_RICH, FLAG_HI_SKY_COOL, FLAG_HI_SKY_LOOK,
    FLAG_HI_TINT_BALANCE, FLAG_HI_WATER_LOOK, FLAG_HI_WATER_SEC, FLAG_HI_WIND_SWAY,
    FLAG_HI_SOFT_SHADOWS, FLAG_HI_COMPACT_SHADE_HIT, FLAG_HI_ISOLATE_GLASS, FLAG_HI_SHADOW_PASS, SPEC_HI_MASK,
};

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

    (FLAG_HI_SNELL_BEND, "FLAG_HI_SNELL_BEND"),

    (FLAG_HI_TINT_BALANCE, "FLAG_HI_TINT_BALANCE"),

    (FLAG_HI_SKY_LOOK, "FLAG_HI_SKY_LOOK"),

    (FLAG_HI_SKY_COOL, "FLAG_HI_SKY_COOL"),
    (FLAG_HI_WATER_LOOK, "FLAG_HI_WATER_LOOK"),
    (FLAG_HI_FOLIAGE_RICH, "FLAG_HI_FOLIAGE_RICH"),
    (FLAG_HI_CANOPY_RELIEF, "FLAG_HI_CANOPY_RELIEF"),
    (FLAG_HI_WIND_SWAY, "FLAG_HI_WIND_SWAY"),

    (FLAG_HI_SOFT_SHADOWS, "FLAG_HI_SOFT_SHADOWS"),
    (FLAG_HI_COMPACT_SHADE_HIT, "FLAG_HI_COMPACT_SHADE_HIT"),
    (FLAG_HI_ISOLATE_GLASS, "FLAG_HI_ISOLATE_GLASS"),
    (FLAG_HI_SHADOW_PASS, "FLAG_HI_SHADOW_PASS"),
];

#[test]
fn every_flag_hi_is_one_power_two() {
    for &(bit, name) in ALL_HI {
        assert!(bit.is_power_of_two(), "{name} = {bit} is not one bit");
    }
}

#[test]
fn every_flag_hi_pair_is_distinct() {
    for (i, &(a, na)) in ALL_HI.iter().enumerate() {
        for &(b, nb) in &ALL_HI[i + 1..] {
            assert_eq!(a & b, 0, "{na} ({a}) overlaps {nb} ({b})");
        }
    }
}

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
