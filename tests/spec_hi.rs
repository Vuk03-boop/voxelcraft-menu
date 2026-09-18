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

fn quoted_names(src: &str, lead: &str, quote: &str) -> std::collections::BTreeSet<String> {
    src.match_indices(lead)
        .map(|(at, _)| {
            let rest = &src[at + lead.len()..];
            let end = rest.find(quote).expect("name is terminated");
            rest[..end].to_string()
        })
        .collect()
}

#[test]
fn every_override_with_a_sibling_flag_is_wired_and_every_wired_name_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_rs = std::fs::read_to_string(root.join("src/render/mod.rs")).expect("mod.rs");
    let mut wgsl = String::new();
    for f in [
        "common", "tile_select", "march", "resolve", "taa", "shaft", "shadow",
    ] {
        wgsl.push_str(
            &std::fs::read_to_string(root.join(format!("src/render/shaders/{f}.wgsl")))
                .unwrap_or_else(|e| panic!("{f}.wgsl: {e}")),
        );
        wgsl.push('\n');
    }
    let overrides = quoted_names(&wgsl, "override SPEC_", ":");
    let flags = quoted_names(&mod_rs, "pub const FLAG_HI_", ":");
    for ov in &overrides {
        let sibling = ov.replacen("SPEC_", "FLAG_HI_", 1);
        if !flags.contains(&sibling) {
            continue;
        }
        assert!(
            mod_rs.contains(&format!("\"{ov}\"")),
            "{sibling} sets a spec_hi bit nobody maps: {ov} is declared but never \
             wired to it, so the flag is dead by construction"
        );
    }
    for wired in quoted_names(&mod_rs, "\"SPEC_", "\"") {
        assert!(
            overrides.contains(&wired),
            "mod.rs wires SPEC_{wired} but no shader declares the override -- a typo \
             here specializes nothing"
        );
    }
}
