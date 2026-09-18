#![allow(clippy::unnecessary_cast)]

fn predicate_v1(wl: u32, ly: u32) -> bool {
    wl < 15 && ly > wl
}

fn predicate_v2(wl: u32, ly: u32, rd: (f32, f32, f32)) -> bool {
    let (dx, dy, dz) = rd;
    if wl >= 15 || (ly as f32) <= wl as f32 {
        return false;
    }
    if dy >= 0.0 {
        return true;
    }
    let h = dx.abs().max(dz.abs());
    h > 1e-6 && (ly as f32 - wl as f32) > 16.0 * (dy.abs() / h)
}

fn transit_is_unsafe(wl: u32, ly: u32, rd: (f32, f32, f32)) -> bool {
    let (dx, dy, dz) = rd;
    let h = dx.abs().max(dz.abs());
    if h < 1e-6 {
        return true;
    }
    let drop = 16.0 * dy.abs() / h;
    dy < 0.0 && (ly as f32 - drop) <= wl as f32
}

#[test]
fn the_skip_cannot_dive_through_the_slab_it_disclaims() {
    use std::f32::consts::PI;
    let mut unsafe_total = 0u32;
    let mut v1_skipped = 0u32;
    for wl in [4u32, 8, 12] {
        for ly in (wl + 1)..16 {
            for e10 in 0..80i32 {
                let elev = e10 as f32 / 10.0;
                let rd = ((elev * PI / 180.0).cos(), -(elev * PI / 180.0).sin(), 0.0);
                if !transit_is_unsafe(wl, ly, rd) {
                    continue;
                }
                unsafe_total += 1;
                if predicate_v1(wl, ly) {
                    v1_skipped += 1;
                }
                assert!(
                    !predicate_v2(wl, ly, rd),
                    "skip allowed inside the slab's reach: wl {wl}, ly {ly}, slope {elev} deg"
                );
            }
        }
    }

    assert!(unsafe_total >= 100, "the unsafe set must be non-trivial");
    assert!(
        v1_skipped == unsafe_total,
        "every unsafe transit was skippable under the old predicate ({v1_skipped}/{unsafe_total})\n        -- that is what the navy tiles were"
    );
}

#[test]
fn the_horizontal_ray_keeps_the_skip_the_entry_was_written_for() {
    use std::f32::consts::PI;
    let mut kept = 0u32;
    let mut total = 0u32;
    for (wl, lyo) in [(4u32, 1u32), (8, 3), (12, 1)] {
        for e10 in 0..150i32 {
            let elev = e10 as f32 / 10.0;
            let ly = (wl + lyo + (e10 as u32 % 3)).min(15);
            let rd = ((elev * PI / 180.0).cos(), -(elev * PI / 180.0).sin(), 0.0);
            if predicate_v1(wl, ly) {
                total += 1;
                if predicate_v2(wl, ly, rd) {
                    kept += 1;
                }
            }
        }
    }
    assert!(
        kept * 2 > total,
        "the majority of the 0..15-degree fan must keep the skip ({kept}/{total})"
    );
}

#[test]
fn the_degenerate_rays_are_named_by_the_formula() {

    assert!(!predicate_v2(8, 15, (0.0, -1.0, 0.0)));

    assert!(predicate_v2(8, 15, (0.9, 0.0, 0.44)));

    assert!(predicate_v2(8, 15, (0.2, 0.8, 0.1)));

    let s9 = (9.0f32 * core::f32::consts::PI / 180.0).sin();
    let c9 = (9.0f32 * core::f32::consts::PI / 180.0).cos();
    let rd = (c9, -s9, 0.0);
    assert!(!predicate_v2(8, 10, rd), "two above the line cannot skip at 9 degrees");
    assert!(predicate_v2(8, 11, rd), "three above the line still can");
}
