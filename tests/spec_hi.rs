use voxelcraft::render::{
    FLAG_HI_LEGACY_LIGHTING, FLAG_HI_LEGACY_WATER, FLAG_HI_SOFT_HQ, FLAG_HI_DEBUG_A, FLAG_HI_DEBUG_B, FLAG_HI_DEBUG_C, FLAG_HI_SUN_NARROW, FLAG_HI_SUN_WIDE,
    FLAG_HI_CAUSTICS, FLAG_HI_EMITTER_WORLD, FLAG_HI_FULL_MARCH, FLAG_HI_GLASS_REFLECT,
    FLAG_HI_SHORE_FOAM, FLAG_HI_SHORE_WET, FLAG_HI_SKY_SPECULAR, FLAG_HI_SNELL_BEND,
    FLAG_HI_CANOPY_RELIEF, FLAG_HI_FOLIAGE_RICH, FLAG_HI_SKY_COOL, FLAG_HI_SKY_LOOK,
    FLAG_HI_TINT_BALANCE, FLAG_HI_WATER_LOOK, FLAG_HI_WATER_SEC, FLAG_HI_WIND_SWAY,
    FLAG_HI_SOFT_SHADOWS, FLAG_HI_COMPACT_SHADE_HIT, FLAG_HI_ISOLATE_GLASS, FLAG_HI_SHADOW_PASS, FLAG_HI_GLASS_QUAD, SPEC_HI_MASK,
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
    (FLAG_HI_GLASS_QUAD, "FLAG_HI_GLASS_QUAD"),

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

// Spec-hi wiring pairs are drift-prone: the flag file and specialization list come
// from render/mod.rs by explicit name, and a SPEC_ override nobody wires in is the
// kind of dead bit this suite exists to catch. Both directions are pinned:
// every SPEC_* override that has a FLAG_HI_* sibling must be wired, and every
// quoted "SPEC_*" name the wiring passes must be an override some shader declares.
//
// Line-anchored scans, no byte-index slicing: earlier draft of this pin scanned raw
// quote matches with .expect("name is terminated"), which the final quote in
// mod.rs provoked into a panic at the .expect. Lines cannot lie about closure.
fn wired_spec_names(src: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for line in src.lines() {
        let t = line.trim();
        let rest = t.strip_prefix("(\"").or_else(|| t.strip_prefix("\""));
        if let Some(r) = rest {
            if let Some(end) = r.find('"') {
                let name = &r[..end];
                if name.starts_with("SPEC_") {
                    out.insert(name.to_string());
                }
            }
        }
    }
    out
}

fn const_flag_names(src: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for line in src.lines() {
        if let Some(r) = line.trim().strip_prefix("pub const ") {
            if let Some(end) = r.find(':') {
                let name = &r[..end];
                if name.starts_with("FLAG_HI_") {
                    out.insert(name.to_string());
                }
            }
        }
    }
    out
}

fn shader_override_names(src: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for line in src.lines() {
        if let Some(r) = line.trim().strip_prefix("override ") {
            if let Some(end) = r.find(':') {
                let name = &r[..end];
                if name.starts_with("SPEC_") {
                    out.insert(name.to_string());
                }
            }
        }
    }
    out
}

#[test]
fn every_override_with_a_sibling_flag_is_wired_and_every_wired_name_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mod_rs = std::fs::read_to_string(root.join("src/render/mod.rs")).expect("mod.rs");
    let mut wgsl = String::new();
    for f in [
        "common.wgsl", "tile_select.wgsl", "march.wgsl", "resolve.wgsl",
        "taa.wgsl", "shaft.wgsl", "shadow.wgsl",
    ] {
        wgsl.push_str(
            &std::fs::read_to_string(root.join(format!("src/render/shaders/{f}")))
                .unwrap_or_else(|e| panic!("{f}.wgsl: {e}")),
        );
        wgsl.push('\n');
    }
    let overrides = shader_override_names(&wgsl);
    let flags = const_flag_names(&mod_rs);
    let wired = wired_spec_names(&mod_rs);
    for ov in &overrides {
        let sibling = ov.replacen("SPEC_", "FLAG_HI_", 1);
        if !flags.contains(&sibling) {
            continue;
        }
        assert!(
            wired.contains(ov),
            "{sibling} exists but no pipeline wired {ov}",
        );
    }
    for w in &wired {
        assert!(
            overrides.contains(w),
            "mod.rs wired {w} but no shader declares that override",
        );
    }
}
