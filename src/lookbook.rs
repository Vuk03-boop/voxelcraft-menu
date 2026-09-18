//! Batch 98's lookbook: the flagship views against the look-arm matrix, composed into one
//! labeled PNG for a comparison pass the user hands to an AI -- so the *sheet itself* names
//! every tile, because a contact sheet with a sidecar manifest is a manifest the AI never
//! reads.
//!
//! Three views, chosen from the fixture's own cameras (the batch-22 law: a camera somebody
//! already verified shows content, not a guess at one):
//!
//! - **SEA-SUNSET** is `coastline`'s camera at its own time -- down the coast into a low
//!   sun (0.74), water and banks in frame, the expensive water/sky mix.
//! - **SUNRISE-MOUNTAINS** is `lod`'s camera at its own time -- 240 blocks over the terrain
//!   at 0.26 (sunrise, `sun()`'s horizon), mountains and the transition band.
//! - **MIDDAY-REEDS** is coastline's own yaw at noon (0.50) but waist-height
//!   (`--cam-height 2`) and nearly level -- the first-98 batch's float cam filled the frame
//!   with a near hillside, and both references are shot waist-height inside the world, so
//!   this view is the sheet's eye for the G2 grass, the bank reeds, and the midday blue
//!   `--sky-cool` buys (framing proven at the first hardware render, refined then).
//!
//! Fourteen combos: the batch-97 look arms singly, G2's ground-cover pair as one tile
//! (the designation was repaired at batch 102 -- it now arms `--grass-dense` and
//! `--wind-sway`, having spent three rounds arming the 97d/97e albedo pair under G2's
//! name), then everything at once, then the strength knobs the user asked after -- fog's
//! density ladder, godrays' strength ladder and cine's dial, which *are* still in the
//! tree (`--fog-density`, `--godray-strength`, `--grade-strength`; defaults 5.0e-4, 1.0
//! and 1.0 respectively) -- and batch 102a's penumbra arm, which the bench refused.
//!
//! Everything in this file is GPU-free by design: the tables, the apply functions, the
//! compose and the font. The renderer never enters the module, so `tests/batch98.rs` can
//! pin the whole shape of a run without touching a device.

use crate::config::{parse_from, Config};

/// One lookbook view: a name (drawn on the sheet) and the camera arguments (parsed by the
/// same `parse_from` the fixture uses, so a renamed flag fails loudly here too).
pub struct LookView {
    pub name: &'static str,
    pub about: &'static str,
    pub args: &'static [&'static str],
}

pub const VIEWS: [LookView; 4] = [
    LookView {
        name: "SEA-SUNSET",
        about: "coastline's camera at 0.74: down the coast into a low sun, water and banks",
        args: &[
            "--time",
            "0.74",
            "--cam-height",
            "24",
            "--cam-pitch",
            "-6",
            "--cam-yaw",
            "143",
        ],
    },
    LookView {
        name: "SUNRISE-MOUNTAINS",
        about: "lod's camera at 0.26: 240 up over the mountains at sunrise",
        args: &[
            "--time",
            "0.26",
            "--cam-yaw",
            "37",
            "--cam-pitch",
            "-20",
            "--cam-height",
            "240",
        ],
    },
    LookView {
        name: "MIDDAY-REEDS",
        about: "coastline's yaw lowered to the references' eye: waist in the bank grass at noon",
        args: &[
            "--time",
            "0.50",
            "--cam-height",
            "2",
            "--cam-pitch",
            "-3",
            "--cam-yaw",
            "143",
        ],
    },
    // Batch 101j. The hardware round's own measurement: at MIDDAY-REEDS the G2 arm
    // moves 20,007 px (MAE 0.47) because the frame is ~97% water and sky -- G2 was
    // judged invisible against a camera that never points at it. At `default` the same
    // arm moves 433,502 (MAE 4.88). The meadow-side camera: same noon, inland-facing
    // yaw along the shore, and enough height to see the carpet *and* the tussock
    // profile. This is the view the G2 arm's tiles must be read at; drive the row from
    // the meadow position (`default`'s station) rather than the coastline's.
    LookView {
        name: "MIDDAY-MEADOW",
        about: "the G2 tribunal camera: noon, inland-facing, meadow-level -- the arm's real measured reach",
        args: &[
            "--time",
            "0.50",
            "--cam-height",
            "14",
            "--cam-pitch",
            "-9",
            "--cam-yaw",
            "323",
        ],
    },
];

/// The camera-plus-time channels a view is allowed to set. A lookbook view is a camera, by
/// the fixture's own batch-22 sentence; reaching past these six fields would be a lookbook
/// that changes the world, and the sheet would stop being a comparison of frames of one
/// world. The test in batch98 pins the whitelist **reading this table**, so the table is
/// the mirror the assertion looks at, not a second list kept in sync by eye.
pub const VIEW_CHANNELS: [&str; 6] = [
    "--time",
    "--cam-height",
    "--cam-yaw",
    "--cam-pitch",
    "--cam-submerge",
    "--cloud-cover",
];

/// Apply a view to a configuration the way the parser would -- then copy only the camera
/// channels, so the caller's seed, resolution and world knobs survive. Unknown arguments
/// poison the caller (a misspelled entry is not a camera, it is the default camera in
/// disguise, the fixture's worst failure mode); returns false then.
pub fn apply_view(cfg: &mut Config, view: &LookView) -> bool {
    let args: Vec<String> = view.args.iter().map(|s| s.to_string()).collect();
    let (vc, _, unknown) = parse_from(&args);
    if !unknown.is_empty() {
        return false;
    }
    cfg.time_of_day = vc.time_of_day;
    cfg.cam_height = vc.cam_height;
    cfg.cam_yaw = vc.cam_yaw;
    cfg.cam_pitch = vc.cam_pitch;
    cfg.cam_submerge = vc.cam_submerge;
    cfg.clouds.cover = vc.clouds.cover;
    true
}

/// One settings combination: a short label (drawn on the tile), a sentence of what it is,
/// and the mutation itself as a plain function pointer -- no closures, no capture, so the
/// table is data the test suite can walk.
pub struct Combo {
    pub label: &'static str,
    pub about: &'static str,
    pub apply: fn(&mut Config),
}

/// The fourteen-tile matrix -- over **four** views (batch 101j added MIDDAY-MEADOW
/// after hardware found the G2 arm under-measured at every prior camera; batch 102a
/// added SOFT-SHADOWS, the sun-disc penumbra, whose cost claim is exactly what a
/// matrix cell exists to measure). Off-by-default arms keep the group honest: BASE is
/// the control every other tile reads against, ALL-FIVE is the "all of it" ceiling,
/// and the strength ladders (fog, godrays, cine from batch 101) bracket the amounts a
/// by-eye pass might settle between.
pub const COMBOS: [Combo; 14] = [
    Combo {
        label: "BASE",
        about: "no look arms: the control",
        apply: |_| {},
    },
    Combo {
        label: "SKY-COOL",
        about: "batch 97a: cornflower midday sky",
        apply: |c| c.sky_cool = true,
    },
    Combo {
        label: "CINE",
        about: "batch 97b: the filmic print",
        apply: |c| c.grade_cine = true,
    },
    Combo {
        label: "CINE-HALF",
        about: "batch 101: cine at --grade-strength 0.5, the sweep verdict's dial",
        apply: |c| {
            c.grade_cine = true;
            c.grade_strength = 0.5;
        },
    },
    Combo {
        label: "WATER-LOOK",
        about: "batch 97c: ripples, murk, bank band",
        apply: |c| c.water_look = true,
    },
    Combo {
        label: "FOLIAGE",
        about: "batch 97d: ground-cover species variety",
        apply: |c| c.foliage_rich = true,
    },
    Combo {
        label: "CANOPY",
        about: "batch 97e: canopy grain",
        apply: |c| c.canopy_relief = true,
    },
    Combo {
        label: "G2-PAIR",
        // Batch 102's designation repair: this tile armed FOLIAGE+CANOPY (the batch-97
        // albedo pair, roadmap G4's subject) while wearing G2's name since 658e31d --
        // two tiles already on this sheet, duplicated, and MIDDAY-MEADOW's tribunal
        // absent. G2 is ground cover: density and its movement. `tests/batch98.rs`'s
        // two mirrors gained both fields in the same commit, so the tile's world
        // mutation is sanctioned there rather than invisible here.
        about: "G2's two halves together: dense tufts and their sway",
        apply: |c| {
            c.grass_dense = true;
            c.wind_sway = true;
        },
    },
    Combo {
        label: "ALL-FIVE",
        about: "every batch-97 arm at once",
        apply: |c| {
            c.sky_cool = true;
            c.grade_cine = true;
            c.water_look = true;
            c.foliage_rich = true;
            c.canopy_relief = true;
        },
    },
    Combo {
        label: "FOG-LOW",
        about: "--fog-density at half",
        apply: |c| c.fog.density *= 0.5,
    },
    Combo {
        label: "FOG-DENSE",
        about: "--fog-density doubled",
        apply: |c| c.fog.density *= 2.0,
    },
    Combo {
        label: "GODRAYS-HALF",
        about: "--godray-strength 0.5",
        apply: |c| c.godrays.strength = 0.5,
    },
    Combo {
        label: "GODRAYS-OFF",
        about: "--godray-strength 0 (batch 11's own control)",
        apply: |c| c.godrays.strength = 0.0,
    },
    Combo {
        label: "SOFT-SHADOWS",
        about: "batch 102a: sun-disc penumbra, blocker-distance-tapered cone taps",
        apply: |c| c.soft_shadows = true,
    },
];

// ---------------------------------------------------------------------------
// Composing the sheet
// ---------------------------------------------------------------------------

pub const COLS: u32 = 4;
/// Per-tile label strip height, in sheet pixels.
pub const LABEL_H: u32 = 16;
/// Per-view group header strip height.
pub const GROUP_H: u32 = 26;
/// Tiles render at the capture's full resolution and downsample by this factor for the
/// sheet -- the capture pipeline stays the fixture's, the sheet stays an AI-handable size.
pub const SCALE: u32 = 2;

/// One rendered tile: label plus full-resolution RGBA pixels.
pub struct Tile {
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Where a group strip ends and its tiles begin, for the label painter: strips are
/// 0x24/0x2B/0x35 (dark navy), tile labels the same family, pixels themselves untouched.
fn strip_bg(px: &mut [u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32) {
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * w + x) * 4) as usize;
            px[i] = 0x20;
            px[i + 1] = 0x28;
            px[i + 2] = 0x33;
            px[i + 3] = 0xFF;
        }
    }
}

fn put_px(px: &mut [u8], sheet_w: u32, x: u32, y: u32, shade: u8) {
    // The fork's hardware find (batch-99 re-run): past-edge ink must clip, not wrap --
    // with x >= sheet_w a linear index `y * w + x` spills into the *next row*, so the
    // label strip of a right-edge tile painted into the top of the tile beside/below
    // it. That was the compose test's mystery [204,204,204]: on a sheet exactly four
    // tiles wide the last label starts at x == w, and its stamp wrapped two rows down
    // into tile 0's data. A glyph cell past the edge is simply lost; every strip is
    // wider than the labels it was authored for.
    if x >= sheet_w {
        return;
    }
    let i = ((y * sheet_w + x) * 4) as usize;
    px[i] = shade;
    px[i + 1] = shade;
    px[i + 2] = shade;
    px[i + 3] = 0xFF;
}

/// One character cell is 5x7 plus a one-pixel gutter; drawn at `s`x so the strip labels
/// stay legible after whatever viewer the sheet passes through downscales by another two.
pub fn draw_text(px: &mut [u8], sheet_w: u32, x0: u32, y0: u32, text: &str, s: u32, shade: u8) {
    let mut cx = x0;
    for ch in text.chars() {
        let glyph = glyph(ch);
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    for dy in 0..s {
                        for dx in 0..s {
                            put_px(
                                px,
                                sheet_w,
                                cx + col as u32 * s + dx,
                                y0 + row as u32 * s + dy,
                                shade,
                            );
                        }
                    }
                }
            }
        }
        cx += 6 * s;
    }
}

/// Compose one view's tiles into a vertical segment: a group strip carrying the view's
/// name, then COLS-wide rows of (label strip + downsampled tile).
pub fn compose_group(view: &LookView, tiles: &[Tile]) -> (u32, u32, Vec<u8>) {
    let tw = tiles[0].width / SCALE;
    let th = tiles[0].height / SCALE;
    let rows = (tiles.len() as u32).div_ceil(COLS);
    let w = COLS * tw;
    let h = GROUP_H + rows * (LABEL_H + th);
    let mut px = vec![0u8; (w * h * 4) as usize];
    strip_bg(&mut px, w, 0, 0, w, GROUP_H);
    draw_text(&mut px, w, 8, (GROUP_H - 14) / 2, view.name, 2, 0xEE);
    for (n, t) in tiles.iter().enumerate() {
        let col = n as u32 % COLS;
        let row = n as u32 / COLS;
        let x = col * tw;
        let y = GROUP_H + row * (LABEL_H + th);
        strip_bg(&mut px, w, x, y, x + tw, y + LABEL_H);
        draw_text(&mut px, w, x + 4, y + (LABEL_H - 14) / 2, &t.label, 2, 0xCC);
        for py in 0..th {
            for pxx in 0..tw {
                let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
                for dy in 0..SCALE {
                    for dx in 0..SCALE {
                        let sx = (pxx * SCALE + dx).min(t.width - 1);
                        let sy = (py * SCALE + dy).min(t.height - 1);
                        let si = ((sy * t.width + sx) * 4) as usize;
                        r += t.pixels[si] as u32;
                        g += t.pixels[si + 1] as u32;
                        b += t.pixels[si + 2] as u32;
                        a += t.pixels[si + 3] as u32;
                    }
                }
                let n_ = SCALE * SCALE;
                let di = (((y + LABEL_H + py) * w + x + pxx) * 4) as usize;
                px[di] = (r / n_) as u8;
                px[di + 1] = (g / n_) as u8;
                px[di + 2] = (b / n_) as u8;
                px[di + 3] = (a / n_) as u8;
            }
        }
    }
    (w, h, px)
}

/// Stack the three view segments into one sheet. All groups share a width by construction
/// (same tile grid), so this is a concat with no resampling.
pub fn compose_sheet(groups: &[(u32, u32, Vec<u8>)]) -> (u32, u32, Vec<u8>) {
    let w = groups[0].0;
    let h: u32 = groups.iter().map(|g| g.1).sum();
    let mut px = vec![0u8; (w * h * 4) as usize];
    let mut y = 0u32;
    for (gw, gh, gp) in groups {
        assert_eq!(*gw, w, "all view groups share the sheet width");
        px[(y * w * 4) as usize..((y + gh) * w * 4) as usize].copy_from_slice(gp);
        y += gh;
    }
    (w, h, px)
}

// ---------------------------------------------------------------------------
// The 5x7 font (the sheet's whole dependency)
// ---------------------------------------------------------------------------

/// The uppercase-cutout charset. A label outside it falls to '?' (tested, so a renamed
/// arm cannot fail silently -- worth it, since a wrong glyph would lie about which tile
/// is which to exactly the reader this is for).
pub fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x0F, 0x01, 0x01, 0x01, 0x01, 0x11, 0x0E],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F],
        '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        ' ' => [0x00; 7],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        _ => [0x1F, 0x11, 0x01, 0x06, 0x04, 0x00, 0x04],
    }
}

/// Does the charset cover this text? The batch98 pin walks every fixed label and view
/// name through it, so a renamed combo that sneak in a lowercase letter fails before the
/// sheet ever lies.
pub fn covered(text: &str) -> bool {
    text.chars().all(|c| {
        matches!(c,
            'A'..='Z' | '0'..='9' | ' ' | '-' | '.' | ':' | '+')
    })
}

