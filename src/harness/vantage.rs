use super::metric::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {

    Water,

    Land,

    Sky,

    Mixed,
}

impl Content {
    pub fn label(&self) -> &'static str {
        match self {
            Content::Water => "water",
            Content::Land => "land",
            Content::Sky => "sky",
            Content::Mixed => "mixed",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Crop {
    pub name: &'static str,
    pub content: Content,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Crop {

    pub fn rect(&self, w: u32, h: u32) -> Rect {
        Rect {
            x0: (self.x0 * w as f32).round() as u32,
            y0: (self.y0 * h as f32).round() as u32,
            x1: (self.x1 * w as f32).round() as u32,
            y1: (self.y1 * h as f32).round() as u32,
        }
    }
}

pub struct Vantage {
    pub name: &'static str,

    pub args: &'static [&'static str],
    pub about: &'static str,
    pub crops: &'static [Crop],
}

impl Vantage {
    pub fn crop(&self, name: &str) -> Option<&Crop> {
        self.crops.iter().find(|c| c.name == name)
    }
}

pub const STD_WIDTH: u32 = 1280;
pub const STD_HEIGHT: u32 = 720;

pub const VANTAGES: &[Vantage] = &[
    Vantage {
        name: "default",
        args: &["--time", "0.30"],
        about: "the default camera: a grassy mountain filling the frame, water at the right edge",
        crops: &[Crop {
            name: "slope",
            content: Content::Land,
            x0: 0.05,
            y0: 0.45,
            x1: 0.55,
            y1: 0.95,
        }],
    },
    Vantage {
        name: "coastline",
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
        about: "down a coastline into a low sun -- the expensive vantage for anything a \
                secondary ray reads, since `shade_water` calls `sky_color` unconditionally",
        crops: &[
            Crop {
                name: "water",
                content: Content::Water,
                x0: 0.42,
                y0: 0.64,
                x1: 0.78,
                y1: 0.95,
            },
            Crop {
                name: "sky",
                content: Content::Sky,
                x0: 0.05,
                y0: 0.02,
                x1: 0.35,
                y1: 0.16,
            },
        ],
    },
    Vantage {
        name: "open-sea",
        args: &[
            "--time",
            "0.74",
            "--cam-submerge",
            "-60",
            "--cam-pitch",
            "-8",
            "--cam-yaw",
            "143",
        ],
        about: "sixty blocks above open water: batch 19's vantage, where the grazing footprint \
                correction is worth 15.9x at the horizon",
        crops: &[Crop {
            name: "water",
            content: Content::Water,
            x0: 0.32,
            y0: 0.64,
            x1: 0.68,
            y1: 0.95,
        }],
    },
    Vantage {
        name: "shore",
        args: &[
            "--cam-submerge",
            "-1",
            "--cam-yaw",
            "-219",
            "--cam-pitch",
            "-4",
        ],
        about: "one block above the sea looking along it -- nothing but shallow water, which is \
                the only water batch 20 does any work for",
        crops: &[Crop {
            name: "water",
            content: Content::Water,
            x0: 0.38,
            y0: 0.62,
            x1: 0.95,
            y1: 0.97,
        }],
    },
    Vantage {
        name: "lattice",
        args: &["--cam-pitch", "-10", "--cam-yaw", "-122"],
        about: "batch 21's measurement vantage, over a wide shallow lagoon. Its doc calls this \
                \"eye at the waterline\" and passes `--cam-submerge 0`, which is the off value: \
                the eye is at the default height of 40. The arguments here are the ones that \
                were run, with the no-op dropped",
        crops: &[Crop {
            name: "water",
            content: Content::Water,
            x0: 0.22,
            y0: 0.60,
            x1: 0.68,
            y1: 0.90,
        }],
    },
    Vantage {
        name: "terraces",
        args: &["--cam-submerge", "-2"],
        about: "two blocks above the sea, pitched down over a terraced bottom: batch 21b's \
                vantage, where the refraction's per-voxel path quantisation shows as salmon \
                diamonds on the submerged floor and the dry terraces beside them stay clean",

        crops: &[
            Crop {
                name: "water",
                content: Content::Water,
                x0: 0.55,
                y0: 0.78,
                x1: 0.98,
                y1: 0.98,
            },
            Crop {
                name: "dry",
                content: Content::Land,
                x0: 0.05,
                y0: 0.10,
                x1: 0.55,
                y1: 0.50,
            },
        ],
    },
    Vantage {
        name: "lod",
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
        about: "240 blocks up: the only vantage that puts a meaningful share of the frame in an \
                LOD transition band. At eye level a fade A/B moves 0.05% of pixels; here it \
                moves 13%",
        crops: &[],
    },
    Vantage {
        name: "low-sun",
        args: &["--time", "0.26", "--cam-yaw", "37", "--cam-pitch", "0"],
        about: "into the sun at the horizon. `sun()` reaches the horizon at 0.26, not at the \
                0.05 an early roadmap claimed -- 0.05 is the middle of the night",
        crops: &[],
    },
    Vantage {
        name: "sky",
        args: &[
            "--time",
            "0.26",
            "--cam-yaw",
            "37",
            "--cam-pitch",
            "20",
            "--cam-height",
            "60",
        ],
        about: "pitched up into a sky-filling frame",
        crops: &[Crop {
            name: "sky",
            content: Content::Sky,
            x0: 0.20,
            y0: 0.05,
            x1: 0.80,
            y1: 0.35,
        }],
    },
    Vantage {
        name: "overcast",
        args: &["--time", "0.50", "--cloud-cover", "0.70"],
        about: "noon under heavy cover",
        crops: &[],
    },
    Vantage {
        name: "underwater",
        args: &["--time", "0.26", "--cam-submerge", "3"],
        about: "the eye three blocks under the surface: the only way to reach the underwater \
                medium headlessly",
        crops: &[],
    },
    Vantage {
        name: "sea-horizon",

        args: &[
            "--time",
            "0.35",
            "--cam-submerge",
            "3",
            "--cam-yaw",
            "215",
            "--cam-pitch",
            "0",
        ],
        about: "a submerged eye looking along the horizontal across open water at a chain of \
                wooded islands: the only vantage where the reach a submerged camera stops \
                testing chunks at decides anything, and the only one that is not bit-exact \
                under `--no-water-far`",

        crops: &[
            Crop {

                name: "horizon",
                content: Content::Mixed,
                x0: 0.00,
                y0: 0.43,
                x1: 1.00,
                y1: 0.52,
            },
            Crop {

                name: "deep",
                content: Content::Mixed,
                x0: 0.00,
                y0: 0.54,
                x1: 1.00,
                y1: 0.74,
            },
        ],
    },
    Vantage {
        name: "deep-water",

        args: &[
            "--time",
            "0.35",
            "--cam-submerge",
            "24",
            "--cam-yaw",
            "215",
            "--cam-pitch",
            "0",
        ],
        about: "an eye 24 blocks down over open sea floor, looking along the horizontal: the                 only vantage deep enough for the sky flood to have run out, and so the only                 one where the dark-water rung of the submerged reach decides anything",
        crops: &[
            Crop {

                name: "graze",
                content: Content::Mixed,
                x0: 0.00,
                y0: 0.34,
                x1: 1.00,
                y1: 0.54,
            },
            Crop {

                name: "floor",
                content: Content::Mixed,
                x0: 0.00,
                y0: 0.62,
                x1: 1.00,
                y1: 0.96,
            },
        ],
    },
    Vantage {
        name: "cave",
        args: &["--time", "0.30", "--cam-height", "-20"],
        about: "inside a real air pocket, found by search rather than by hoping",
        crops: &[],
    },
    Vantage {
        name: "night",
        args: &["--time", "0.05"],
        about: "the middle of the night",
        crops: &[],
    },
    Vantage {
        name: "edits",

        args: &["--demo-edits", "--cam-height", "14", "--cam-pitch", "-20"],
        about: "a lit structure built through the normal edit path, which is the only headless \
                capture that exercises one -- from a camera low enough to see it, which the \
                default is not",
        crops: &[],
    },
    Vantage {
        name: "glass",

        args: &["--demo-glass", "--cam-height", "14", "--cam-pitch", "-20"],
        about: "a glasshouse built through the normal edit path: the only capture holding a \
                transmissive block, and the only one whose interior is lit through a closed \
                roof rather than through a hole in it",
        crops: &[Crop {
            name: "pane",

            content: Content::Land,
            x0: 0.33,
            y0: 0.40,
            x1: 0.70,
            y1: 0.89,
        }],
    },
    Vantage {
        name: "glass-sea",

        args: &[
            "--demo-glass",
            "--cam-submerge",
            "1",
            "--cam-yaw",
            "143",
            "--cam-pitch",
            "-25",
        ],
        about: "a glasshouse standing in the shallows: the only capture in which a pane has                 water behind it, and therefore the only one that can see how water seen                 *through* glass is shaded",
        crops: &[Crop {

            name: "wet-pane",
            content: Content::Mixed,
            x0: 0.30,
            y0: 0.45,
            x1: 0.72,
            y1: 0.92,
        }],
    },
    Vantage {
        name: "lamps",

        args: &["--demo-lamps", "--time", "0.30", "--cam-height", "-20", "--cam-yaw", "0", "--cam-pitch", "0"],
        about: "a sealed chamber lit by four emitters of four different colours, which is the \
                only capture in the set that can tell coloured block light from a warm tint \
                applied to a scalar one",
        crops: &[Crop {

            name: "mixing-wall",
            content: Content::Land,
            x0: 0.38,
            y0: 0.26,
            x1: 0.62,
            y1: 0.62,
        }],
    },
    Vantage {
        name: "canopy",

        args: &[
            "--cam-height",
            "8",
            "--cam-pitch",
            "14",
            "--cam-yaw",
            "300",
            "--time",
            "0.35",
        ],
        about: "a wooded crest against open sky: the only vantage where a canopy is a \
                silhouette rather than a mass, which is what `LEAF_FILL` decides and what \
                roadmap 38d had no crop for",
        crops: &[Crop {
            name: "skyline",

            content: Content::Mixed,
            x0: 0.46,
            y0: 0.33,
            x1: 0.66,
            y1: 0.66,
        }],
    },
    Vantage {
        name: "offscreen-shadow",

        args: &[
            "--time",
            "0.32",
            "--cam-yaw",
            "265",
            "--cam-height",
            "80",
        ],
        about: "a coastal slope 133 degrees round from the sun: the only vantage where the \
                terrain casting the shadow is off screen, which is what `--no-offscreen-shadows` \
                needs and what no single still can otherwise show",
        crops: &[Crop {
            name: "shadowed-slope",

            content: Content::Land,
            x0: 0.80,
            y0: 0.55,
            x1: 1.00,
            y1: 0.95,
        }],
    },
];

pub fn find(name: &str) -> Option<&'static Vantage> {
    VANTAGES.iter().find(|v| v.name == name)
}

pub fn select(names: &str) -> Result<Vec<&'static Vantage>, String> {
    if names.trim().is_empty() {
        return Ok(VANTAGES.iter().collect());
    }
    names
        .split(',')
        .map(|n| find(n.trim()).ok_or_else(|| format!("no vantage named `{}`", n.trim())))
        .collect()
}
