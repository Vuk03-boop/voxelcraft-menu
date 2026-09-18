pub type BlockId = u16;

pub const AIR: BlockId = 0;
pub const STONE: BlockId = 1;
pub const DIRT: BlockId = 2;
pub const GRASS: BlockId = 3;
pub const SAND: BlockId = 4;
pub const GRAVEL: BlockId = 5;
pub const LOG: BlockId = 6;
pub const LEAVES: BlockId = 7;
pub const PLANKS: BlockId = 8;
pub const COBBLE: BlockId = 9;
pub const GLASS: BlockId = 10;
pub const GLOWSTONE: BlockId = 11;
pub const BEDROCK: BlockId = 12;
pub const WATER: BlockId = 13;

pub const SNOW: BlockId = 14;
pub const PODZOL: BlockId = 15;
pub const PINE_LEAVES: BlockId = 16;
pub const LICHEN: BlockId = 17;

pub const TALL_GRASS: BlockId = 18;

pub const MEADOW: BlockId = 19;

pub const MEADOW_SIDE: BlockId = 23;

pub const AMBER_LAMP: BlockId = 20;
pub const AZURE_LAMP: BlockId = 21;
pub const VERDANT_LAMP: BlockId = 22;

pub const GRASS_TALL: BlockId = 24;
pub const REEDS: BlockId = 25;
pub const BLOCK_COUNT: usize = 26;

pub mod tex {
    pub const STONE: u32 = 0;
    pub const DIRT: u32 = 1;
    pub const GRASS_TOP: u32 = 2;
    pub const GRASS_SIDE: u32 = 3;
    pub const SAND: u32 = 4;
    pub const GRAVEL: u32 = 5;
    pub const LOG_SIDE: u32 = 6;
    pub const LOG_TOP: u32 = 7;
    pub const LEAVES: u32 = 8;
    pub const PLANKS: u32 = 9;
    pub const COBBLE: u32 = 10;
    pub const GLASS: u32 = 11;
    pub const GLOWSTONE: u32 = 12;
    pub const BEDROCK: u32 = 13;
    pub const WATER: u32 = 14;
    pub const SNOW: u32 = 15;
    pub const PODZOL: u32 = 16;
    pub const PINE_LEAVES: u32 = 17;
    pub const LICHEN: u32 = 18;
    pub const TALL_GRASS: u32 = 19;
    pub const MEADOW_TOP: u32 = 20;
    pub const AMBER_LAMP: u32 = 21;
    pub const AZURE_LAMP: u32 = 22;
    pub const VERDANT_LAMP: u32 = 23;

    pub const MEADOW_SIDE: u32 = 24;

    pub const GRASS_TALL: u32 = 25;
    pub const REEDS: u32 = 26;
    pub const COUNT: u32 = 27;
}

pub mod perm {

    pub const NONE: u32 = 0;

    pub const FLIP_U: u32 = 1;

    pub const D4: u32 = 2;
}

pub const PERM_CLASS: [u32; tex::COUNT as usize] = {
    let mut c = [perm::NONE; tex::COUNT as usize];

    c[tex::DIRT as usize] = perm::D4;
    c[tex::GRASS_TOP as usize] = perm::D4;
    c[tex::SAND as usize] = perm::D4;
    c[tex::PODZOL as usize] = perm::D4;
    c[tex::LEAVES as usize] = perm::D4;
    c[tex::PINE_LEAVES as usize] = perm::D4;
    c[tex::MEADOW_TOP as usize] = perm::D4;

    c[tex::MEADOW_SIDE as usize] = perm::FLIP_U;

    c[tex::GRASS_SIDE as usize] = perm::FLIP_U;
    c
};

pub const FACE_LAYER_BITS: u32 = 16;
pub const FACE_LAYER_MASK: u32 = (1 << FACE_LAYER_BITS) - 1;

pub fn pack_face(layer: u32) -> u32 {
    debug_assert!(layer <= FACE_LAYER_MASK, "atlas layer outruns the packed field");
    layer | (PERM_CLASS[layer as usize] << FACE_LAYER_BITS)
}

const _: () = assert!(PERM_CLASS[tex::TALL_GRASS as usize] == perm::NONE);

const _: () = assert!(PERM_CLASS[tex::WATER as usize] == perm::NONE);

#[derive(Clone, Copy, Debug)]
pub struct BlockDef {
    pub name: &'static str,

    pub faces: [u32; 6],

    pub solid: bool,

    pub opaque: bool,

    pub foliage: bool,

    pub cutout: bool,

    pub light: [u8; 3],
}

const fn all(t: u32) -> [u32; 6] {
    [t, t, t, t, t, t]
}
const fn column(side: u32, bottom: u32, top: u32) -> [u32; 6] {
    [side, side, bottom, top, side, side]
}

#[rustfmt::skip]
pub const BLOCKS: [BlockDef; BLOCK_COUNT] = [
    BlockDef { name: "air",         faces: all(0),                                          solid: false, opaque: false, foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "stone",       faces: all(tex::STONE),                                 solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "dirt",        faces: all(tex::DIRT),                                  solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "grass",       faces: column(tex::GRASS_SIDE, tex::DIRT, tex::GRASS_TOP), solid: true, opaque: true, foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "sand",        faces: all(tex::SAND),                                  solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "gravel",      faces: all(tex::GRAVEL),                                solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "log",         faces: column(tex::LOG_SIDE, tex::LOG_TOP, tex::LOG_TOP), solid: true, opaque: true, foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "leaves",      faces: all(tex::LEAVES),                                solid: true,  opaque: true,  foliage: false, cutout: true,  light: [0, 0, 0] },
    BlockDef { name: "planks",      faces: all(tex::PLANKS),                                solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "cobblestone", faces: all(tex::COBBLE),                                solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },

    BlockDef { name: "glass",       faces: all(tex::GLASS),                                 solid: true,  opaque: false, foliage: false, cutout: false, light: [0, 0, 0] },

    BlockDef { name: "glowstone",   faces: all(tex::GLOWSTONE),                             solid: true,  opaque: true,  foliage: false, cutout: false, light: [15, 13, 9] },
    BlockDef { name: "bedrock",     faces: all(tex::BEDROCK),                               solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "water",       faces: all(tex::WATER),                                 solid: false, opaque: false, foliage: false, cutout: false, light: [0, 0, 0] },

    BlockDef { name: "snow",        faces: all(tex::SNOW),                                  solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "podzol",      faces: all(tex::PODZOL),                                solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "pine leaves", faces: all(tex::PINE_LEAVES),                           solid: true,  opaque: true,  foliage: false, cutout: true,  light: [0, 0, 0] },
    BlockDef { name: "lichen",      faces: all(tex::LICHEN),                                solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },

    BlockDef { name: "tall grass",  faces: all(tex::TALL_GRASS),                            solid: false, opaque: false, foliage: true,  cutout: false, light: [0, 0, 0] },

    BlockDef { name: "meadow",      faces: column(tex::GRASS_SIDE, tex::DIRT, tex::MEADOW_TOP), solid: true, opaque: true, foliage: false, cutout: false, light: [0, 0, 0] },

    BlockDef { name: "amber lamp",  faces: all(tex::AMBER_LAMP),                            solid: true,  opaque: true,  foliage: false, cutout: false, light: [15, 9, 3] },
    BlockDef { name: "azure lamp",  faces: all(tex::AZURE_LAMP),                            solid: true,  opaque: true,  foliage: false, cutout: false, light: [3, 9, 15] },
    BlockDef { name: "verdant lamp",faces: all(tex::VERDANT_LAMP),                          solid: true,  opaque: true,  foliage: false, cutout: false, light: [4, 15, 6] },

    BlockDef { name: "meadow-side", faces: column(tex::MEADOW_SIDE, tex::DIRT, tex::MEADOW_TOP), solid: true, opaque: true, foliage: false, cutout: false, light: [0, 0, 0] },

    BlockDef { name: "grass, tall", faces: all(tex::GRASS_TALL),                            solid: false, opaque: false, foliage: true,  cutout: false, light: [0, 0, 0] },
    BlockDef { name: "reeds",       faces: all(tex::REEDS),                                 solid: false, opaque: false, foliage: true,  cutout: false, light: [0, 0, 0] },
];

const _: () = assert!(BLOCKS[MEADOW_SIDE as usize].faces[0] == tex::MEADOW_SIDE);

const _: () = assert!(BLOCKS[GRASS_TALL as usize].faces[0] == tex::GRASS_TALL);
const _: () = assert!(BLOCKS[REEDS as usize].faces[0] == tex::REEDS);
const _: () = assert!(BLOCKS[GRASS_TALL as usize].foliage && BLOCKS[REEDS as usize].foliage);
const _: () = assert!(REEDS as usize == BLOCK_COUNT - 1);

const _: () = assert!(
    BLOCKS[20].light[0] == 15 && BLOCKS[20].light[1] == 9 && BLOCKS[20].light[2] == 3
);
const _: () = assert!(
    BLOCKS[21].light[0] == 3 && BLOCKS[21].light[1] == 9 && BLOCKS[21].light[2] == 15
);
const _: () = assert!(
    BLOCKS[22].light[0] == 4 && BLOCKS[22].light[1] == 15 && BLOCKS[22].light[2] == 6
);

pub const ALBEDO: [[f32; 3]; BLOCK_COUNT] = [
    [0.1738, 0.1738, 0.1738],
    [0.1738, 0.1738, 0.1738],
    [0.2066, 0.1016, 0.0470],
    [0.0920, 0.2947, 0.0407],
    [0.6170, 0.5245, 0.2573],
    [0.1598, 0.1458, 0.1328],
    [0.2828, 0.1724, 0.0622],
    [0.0389, 0.1941, 0.0226],
    [0.3439, 0.1974, 0.0648],
    [0.2269, 0.2269, 0.2269],
    [0.4397, 0.6282, 0.7365],
    [0.8058, 0.6399, 0.2432],
    [0.0651, 0.0651, 0.0744],
    [0.0097, 0.0931, 0.1470],
    [0.7240, 0.7812, 0.9029],
    [0.0844, 0.0386, 0.0137],
    [0.0153, 0.0848, 0.0386],
    [0.1574, 0.1857, 0.1178],
    [0.0121, 0.0685, 0.0063],
    [0.0317, 0.0936, 0.0152],
    [0.6480, 0.2539, 0.0576],
    [0.1397, 0.2884, 0.6801],
    [0.2356, 0.6124, 0.1454],

    [0.0317, 0.0936, 0.0152],
    [0.0371, 0.0987, 0.0080],
    [0.0266, 0.0444, 0.0064],
];

#[inline]
pub fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

pub const HOTBAR: [BlockId; 10] = [
    STONE, DIRT, GRASS, SAND, PLANKS, LOG, LEAVES, COBBLE, GLASS, GLOWSTONE,
];

pub const SLOTS: usize = HOTBAR.len();

pub type Bar = [BlockId; SLOTS];

pub fn bar_slot_refusal(id: BlockId) -> Option<&'static str> {
    match id {
        AIR => Some("air is not a block you can place"),
        WATER => Some("water would let a run with no sea in it acquire a water pixel"),
        TALL_GRASS | GRASS_TALL | REEDS => {
            Some("a placed tuft is invisible in a build compiled without foliage")
        }
        _ if id as usize >= BLOCK_COUNT => Some("no block has that id"),
        _ => None,
    }
}

#[inline]
pub fn def(id: BlockId) -> &'static BlockDef {
    &BLOCKS[(id as usize).min(BLOCK_COUNT - 1)]
}

#[inline]
pub fn fills_voxel(id: BlockId) -> bool {
    id != AIR
}

pub const FACES_PER_BLOCK: usize = 8;

pub fn face_table() -> Vec<u32> {
    let mut v = Vec::with_capacity(BLOCK_COUNT * FACES_PER_BLOCK);
    for b in BLOCKS.iter() {

        for f in b.faces {
            v.push(pack_face(f));
        }
        v.push(pack_face(b.faces[0]));
        v.push(pack_face(b.faces[0]));
    }
    v
}

#[inline]
pub fn is_foliage(id: BlockId) -> bool {
    def(id).foliage
}

#[inline]
pub fn is_glass(id: BlockId) -> bool {
    id == GLASS
}

pub fn is_emitter(id: BlockId) -> bool {
    def(id).light != [0, 0, 0]
}

pub fn is_cutout(id: BlockId) -> bool {
    def(id).cutout
}

pub fn casts_sun_shadow(id: BlockId) -> bool {
    def(id).opaque && id != GLOWSTONE
}
