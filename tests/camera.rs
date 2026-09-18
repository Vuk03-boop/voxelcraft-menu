//! `math::view_proj` is only useful to the temporal pass if it is the exact inverse of the
//! ray generation in `common.wgsl`: a hit reconstructed for pixel (x, y) has to project
//! back to (x, y). `taa.wgsl` reprojects through it every frame, so these keep it honest,
//! along with the jitter conventions the two shaders have to agree on.

use glam::Vec3;
use voxelcraft::math;

/// The camera basis `main.rs` hands the renderer.
fn basis(yaw_deg: f32, pitch_deg: f32) -> (Vec3, Vec3, Vec3) {
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    let fwd = Vec3::new(y.cos() * p.cos(), p.sin(), y.sin() * p.cos()).normalize();
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd).normalize();
    (fwd, right, up)
}

/// `common.wgsl::ray_dir`, transcribed.
fn ray_dir_jittered(
    px: f32,
    py: f32,
    j: (f32, f32),
    res: (f32, f32),
    b: (Vec3, Vec3, Vec3),
    tan_half: f32,
    aspect: f32,
) -> Vec3 {
    let ndc_x = ((px + j.0) / res.0) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((py + j.1) / res.1) * 2.0;
    (b.0 + b.1 * (ndc_x * tan_half * aspect) + b.2 * (ndc_y * tan_half)).normalize()
}

fn ray_dir(
    px: f32,
    py: f32,
    res: (f32, f32),
    b: (Vec3, Vec3, Vec3),
    tan_half: f32,
    aspect: f32,
) -> Vec3 {
    ray_dir_jittered(px, py, (0.0, 0.0), res, b, tan_half, aspect)
}

/// `taa.wgsl::reproject`, transcribed: clip space to a position on the history grid.
fn to_history_pixel(clip: glam::Vec4, j: (f32, f32), res: (f32, f32)) -> (f32, f32) {
    let ndc = (clip.x / clip.w, clip.y / clip.w);
    (
        (ndc.0 * 0.5 + 0.5) * res.0 - j.0,
        (0.5 - ndc.1 * 0.5) * res.1 - j.1,
    )
}

#[test]
fn view_proj_inverts_ray_dir() {
    let res = (1280.0f32, 720.0f32);
    let aspect = res.0 / res.1;
    let tan_half = (70.0f32.to_radians() * 0.5).tan();
    let pos = Vec3::new(31.5, 92.25, -404.0);
    for &(yaw, pitch) in &[(40.0, -14.0), (0.0, 0.0), (-155.0, 62.0), (91.0, -80.0)] {
        let b = basis(yaw, pitch);
        let m = math::view_proj(pos, b.0, b.1, b.2, tan_half, aspect, 0.05, 2600.0);
        for &(px, py) in &[(0.5, 0.5), (640.5, 360.5), (1279.5, 719.5), (17.5, 700.5)] {
            let rd = ray_dir(px, py, res, b, tan_half, aspect);
            // Anywhere along the ray must land on the same pixel, near plane to far.
            for &t in &[0.2f32, 7.0, 1200.0] {
                let clip = m * (pos + rd * t).extend(1.0);
                assert!(
                    clip.w > 0.0,
                    "point in front of the camera has w = {}",
                    clip.w
                );
                let x = (clip.x / clip.w * 0.5 + 0.5) * res.0;
                let y = (0.5 - clip.y / clip.w * 0.5) * res.1;
                // Walking t blocks out and subtracting the camera back off loses f32
                // precision in proportion to |pos| / t. That, not the matrix, is the floor
                // here, so the pin is a hundredth of a pixel where distance allows it.
                let tol = 0.01 + res.0 * pos.length() * f32::EPSILON / t;
                assert!(
                    (x - px).abs() < tol && (y - py).abs() < tol,
                    "yaw {yaw} t {t}: pixel ({px}, {py}) came back as ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn view_proj_clip_depth_matches_wgpu_convention() {
    let (near, far) = (0.05f32, 2600.0f32);
    let b = basis(23.0, -9.0);
    let pos = Vec3::new(-8.0, 70.0, 12.0);
    let m = math::view_proj(pos, b.0, b.1, b.2, 0.7, 16.0 / 9.0, near, far);
    for (t, want) in [(near, 0.0f32), (far, 1.0)] {
        let clip = m * (pos + b.0 * t).extend(1.0);
        assert!(
            (clip.z / clip.w - want).abs() < 1e-4,
            "depth at {t} was {}",
            clip.z / clip.w
        );
    }
}

/// The property the whole temporal pass rests on: with the camera standing still, the point
/// a jittered ray hit must land back on its own pixel *centre* in the history. It only does
/// that because `taa` subtracts this frame's jitter from the reprojected position -- without
/// that the accumulator would resample itself by the jitter offset every single frame, and
/// the Catmull-Rom filter would grind the image down instead of leaving it alone.
#[test]
fn a_still_camera_reprojects_onto_the_pixel_centre() {
    let res = (1280.0f32, 720.0f32);
    let aspect = res.0 / res.1;
    let tan_half = (70.0f32.to_radians() * 0.5).tan();
    let pos = Vec3::new(31.5, 92.25, -404.0);
    let b = basis(40.0, -14.0);
    let m = math::view_proj(pos, b.0, b.1, b.2, tan_half, aspect, 0.05, 2600.0);
    for phase in 0..math::TAA_PHASES {
        let jv = math::halton_jitter(phase);
        let j = (jv.x, jv.y);
        for &(px, py) in &[
            (0.5f32, 0.5f32),
            (640.5, 360.5),
            (1279.5, 719.5),
            (17.5, 700.5),
        ] {
            let rd = ray_dir_jittered(px, py, j, res, b, tan_half, aspect);
            // A surface hit carries a position; the sky miss carries only a direction, and
            // `taa` reprojects it with w = 0 so the translation drops out. Both have to
            // come back to the same place when the camera has not moved.
            for clip in [m * (pos + rd * 37.0).extend(1.0), m * rd.extend(0.0)] {
                let (hx, hy) = to_history_pixel(clip, j, res);
                assert!(
                    (hx - px).abs() < 0.01 && (hy - py).abs() < 0.01,
                    "phase {phase} jitter {j:?}: pixel ({px}, {py}) came back as ({hx}, {hy})"
                );
            }
        }
    }
}

/// The eight sample positions are the only thing that turns one frame into an antialiased
/// one, so they have to be eight *different* positions inside the pixel, and they have to
/// come back around: the accumulator's effective window is a handful of frames, and a
/// sequence that drifted would never settle.
#[test]
fn halton_jitter_covers_the_pixel_and_repeats() {
    let mut seen: Vec<(f32, f32)> = Vec::new();
    let mut sum = glam::Vec2::ZERO;
    for i in 0..math::TAA_PHASES {
        let j = math::halton_jitter(i);
        assert!(
            j.x > -0.5 && j.x <= 0.5 && j.y > -0.5 && j.y <= 0.5,
            "phase {i} left the pixel: {j:?}"
        );
        assert!(
            j != glam::Vec2::ZERO,
            "phase {i} sits on the unjittered grid and carries nothing new"
        );
        assert!(
            !seen
                .iter()
                .any(|&(x, y)| (x - j.x).abs() < 1e-6 && (y - j.y).abs() < 1e-6),
            "phase {i} repeats an earlier offset: {j:?}"
        );
        seen.push((j.x, j.y));
        sum += j;
        assert_eq!(
            j,
            math::halton_jitter(i + math::TAA_PHASES),
            "the cycle does not close"
        );
    }
    // Off-centre sample positions would drag the converged image off the pixel grid.
    let mean = sum / math::TAA_PHASES as f32;
    assert!(
        mean.length() < 0.1,
        "the eight phases are biased off centre by {mean:?}"
    );
}



