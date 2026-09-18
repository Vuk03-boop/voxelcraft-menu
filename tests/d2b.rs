use glam::Vec3;
use voxelcraft::harness::metric::{self, Img, Rect};
use voxelcraft::lod::{FadeSchedule, LodConfig, Planner, RenderItem, Residency};

fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}
fn smoother(x: f32) -> f32 {
    x * x * x * (x * (6.0 * x - 15.0) + 10.0)
}

fn plan_with(cfg: &LodConfig, cam: Vec3) -> Vec<RenderItem> {
    let mut p = Planner::default();
    let (mut desired, mut render) = (Vec::new(), Vec::new());
    p.plan(cam, cfg, &|_| Residency::Solid, &mut desired, &mut render);
    render
}

#[test]
fn schedules_hit_the_both_endpoints_and_their_own_slopes() {
    for f in [smooth as fn(f32) -> f32, smoother] {
        assert!(f(0.0).abs() < 1e-6);
        assert!((f(1.0) - 1.0).abs() < 1e-6);

        for i in 0..=10 {
            let x = i as f32 / 10.0;
            assert!(
                (f(x) + f(1.0 - x) - 1.0).abs() < 1e-4,
                "schedule not complementary at x={x}"
            );
        }

        assert!(f(0.01) < 0.001, "edge not flat: f(0.01) = {:.5}", f(0.01));
        assert!(f(0.99) > 0.999);
    }

    let h = 0.1;
    let edge = smooth(0.05) - smooth(0.0);
    let edge_sm = smoother(0.05) - smoother(0.0);
    assert!(edge_sm < edge * 0.5, "edge {edge_sm:.5} vs {edge:.5}");
    let mid = smooth(0.5 + h) - smooth(0.5 - h);
    let mid_sm = smoother(0.5 + h) - smoother(0.5 - h);
    assert!(mid_sm > mid * 1.1, "middle {mid_sm:.5} vs {mid:.5}");
}

#[test]
fn plan_carries_the_selected_schedule() {

    let origin = Vec3::new(-450.0, 140.0, 556.0);
    for (schedule, reference) in [
        (FadeSchedule::Smoothstep, smooth as fn(f32) -> f32),
        (FadeSchedule::Smootherstep, smoother),
    ] {
        let cfg = LodConfig {
            fade_schedule: schedule,
            view_distance: 900.0,
            ..Default::default()
        };
        let mut checked = 0;
        for step in 0..40 {
            let cam = origin + Vec3::new(4.0 * step as f32, 0.0, -3.0 * step as f32);
            for it in plan_with(&cfg, cam) {
                if it.fade >= 0.0 {
                    continue;
                }
                let inner = it.key.size() as f32 * cfg.factor;
                let outer = inner * cfg.fade_band;
                let d = it.key.center().distance(cam);
                if d < inner || d > outer {
                    continue;
                }
                let x = (outer - d) / (outer - inner);
                let want = reference(x);
                let got = 1.0 - it.share();
                assert!(
                    (got - want).abs() < 1e-5,
                    "{schedule:?}: fine share {got} but curve says {want} for {it:?} at d={d:.1}"
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 10,
            "{schedule:?}: only {checked} mid-band coarse entries over the sweep"
        );
    }
}

#[test]
fn band_sweep_is_bounded_by_geometry() {
    let cfg = LodConfig::default();

    let centre_offset = 3f32.sqrt() / 4.0;
    let ceiling = |f: f32| 2.0 - 2.0 * centre_offset / f;
    assert!(cfg.fade_band < ceiling(cfg.factor));

    assert!(ceiling(2.0) > 1.566 && ceiling(2.0) < 1.568);
    assert!(ceiling(cfg.factor) - cfg.fade_band > 0.25);
}

#[test]
fn diff_bbox_finds_the_place() {
    let (w, h) = (64u32, 32u32);
    let mut a = Img::new(w, h);
    let mut b = Img::new(w, h);
    assert_eq!(metric::diff_bbox(&a, &b), None);

    let set = |img: &mut Img, x: u32, y: u32, v: u8| {
        let i = ((y * w + x) * 4) as usize;
        img.px[i] = v;
    };
    set(&mut b, 40, 20, 9);
    assert_eq!(
        metric::diff_bbox(&a, &b),
        Some(Rect {
            x0: 40,
            y0: 20,
            x1: 41,
            y1: 21
        })
    );

    set(&mut b, 2, 1, 7);
    assert_eq!(
        metric::diff_bbox(&a, &b),
        Some(Rect {
            x0: 2,
            y0: 1,
            x1: 41,
            y1: 21
        })
    );

    a.px[((5 * w + 5) * 4 + 3) as usize] = 128;
    let c = Img {
        w: w + 1,
        h,
        px: vec![0; ((w + 1) * h * 4) as usize],
    };
    assert_eq!(metric::diff_bbox(&a, &c), None, "size mismatch must be None");
}

#[test]
fn crop_fractions_round_the_way_the_vantage_table_does() {

    let q = [0.1f32, 0.2, 0.3, 0.4];
    for (w, h) in [(1920u32, 1080u32), (1280, 720)] {
        let vantage = voxelcraft::harness::vantage::Crop {
            name: "t",
            content: voxelcraft::harness::vantage::Content::Mixed,
            x0: q[0],
            y0: q[1],
            x1: q[2],
            y1: q[3],
        };
        let expect = vantage.rect(w, h);
        let f = |v: f32, m: u32| (v.clamp(0.0, 1.0) * m as f32).round() as u32;
        let got = Rect {
            x0: f(q[0], w),
            y0: f(q[1], h),
            x1: f(q[2], w),
            y1: f(q[3], h),
        };
        assert_eq!(got, expect, "crop convention drifted at {w}x{h}");
    }
}
