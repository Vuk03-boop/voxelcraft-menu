//! Pins for the axis-aligned-ray guard in the world-level DDAs
//! (docs/audit-triage.md, applied finding).
//!
//! A zero component of `rd` makes `1.0 / rd` infinite; the axis selection
//! then picks the degenerate axis and the march steps on `0 * inf` -- the
//! mechanism behind the historical glass-transport failure. The guard must
//! sit on the DDA's *stepping* path only: positions, normals and `t` keep
//! the raw `rd`, so every ray with no near-zero component marches
//! bit-identically (the flag sweep's 34 EXACT arms depend on that).

fn shader() -> String {
    voxelcraft::render::shader_source()
}

/// The body of the named function, from its `fn ` marker to the next
/// top-level `fn ` in the module -- the window every pin below searches.
fn body<'a>(src: &'a str, marker: &str) -> &'a str {
    let at = src
        .find(marker)
        .unwrap_or_else(|| panic!("{marker} must exist in the shader source"));
    match src[at + marker.len()..].find("\nfn ") {
        Some(end) => &src[at..at + marker.len() + end],
        None => &src[at..],
    }
}

const GUARD: &str = "let rd_dda = select(rd, vec3<f32>(1e-6, 1e-6, 1e-6) * sign(rd + vec3<f32>(1e-9)), abs(rd) < vec3<f32>(1e-6));";

/// The three world-level chunk-stepping DDAs and their markers.
const SITES: [&str; 3] = [
    "fn march_chunk(",
    "fn shadow_ray(",
    "fn shadow_ray_t(",
];

#[test]
fn the_three_world_level_ddas_guard_their_reciprocal() {
    let src = shader();
    for marker in SITES {
        let b = body(&src, marker);
        assert!(
            b.contains(GUARD),
            "{marker}: the 1e-6 reciprocal guard is gone -- an axis-aligned ray \
             gets an infinite plane time back. {b}"
        );
        assert!(
            b.contains("let inv = 1.0 / rd_dda;"),
            "{marker}: the reciprocal must read the guarded direction, not rd. {b}"
        );
        assert!(
            !b.contains("let inv = 1.0 / rd;"),
            "{marker}: the raw reciprocal is back -- the 0 * inf march with it. {b}"
        );
    }
}

#[test]
fn the_step_direction_moves_with_the_guard() {
    let src = shader();
    for marker in SITES {
        let b = body(&src, marker);
        assert!(
            b.contains("let pos_dir = rd_dda > vec3<f32>(0.0);"),
            "{marker}: pos_dir must come from the guarded vector -- with a zero \
             component, raw `rd.y > 0.0` is false while the guarded plane time \
             wins the axis test, and the march crawls down one cell forever. {b}"
        );
    }
}

#[test]
fn positions_and_normals_keep_the_raw_ray() {
    let src = shader();
    let m = body(&src, "fn march_chunk(");
    assert!(
        m.contains("var pos = ro + rd * t;"),
        "march_chunk positions must ride the raw rd -- guarding them would shift \
         every hit by up to 1e-6 and break the documented bit-identity controls"
    );
    assert!(
        !m.contains("rd_dda * t"),
        "march_chunk slipped rd_dda into position math -- the guard belongs to \
         stepping alone"
    );
    let s = body(&src, "fn shadow_ray(");
    assert!(
        s.contains("ro + rd * t_surf"),
        "shadow_ray's sea-plane offset must stay on the raw rd for the same reason"
    );
}

#[test]
fn cutout_march_is_deliberately_unguarded() {
    let src = shader();
    let b = body(&src, "fn cutout_march(");
    assert!(
        b.contains("let inv = 1.0 / rd;") && !b.contains("rd_dda"),
        "cutout_march gained a speculative guard the audit explicitly declined: \
         no axis-aligned ray is known to reach it, and a guard in a hot loop is \
         cost against no demonstrated bug. If a bug IS demonstrated now, the \
         guarded list in this file grows too."
    );
}

#[test]
fn trace_world_keeps_its_own_house_rule() {
    let src = shader();
    let b = body(&src, "fn trace_world(");
    assert!(
        b.contains("select(rd,"),
        "trace_world's selection DDA had the first guard in the family -- the \
         house rule the other three sites were ported from. {b}"
    );
}
