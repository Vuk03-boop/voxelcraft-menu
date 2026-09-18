//! Block registry: ids, per-face textures, physical properties.

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
// Batch 9's surfaces. Ids are appended, never inserted: `WATER == 13` is mirrored by hand
// in `common.wgsl` and pinned by a `const _: () = assert!` in `render/mod.rs`.
pub const SNOW: BlockId = 14;
pub const PODZOL: BlockId = 15;
pub const PINE_LEAVES: BlockId = 16;
pub const LICHEN: BlockId = 17;
// Batch 14's ground cover. Appended, never inserted, same as every id above it.
pub const TALL_GRASS: BlockId = 18;
// Batch 18's coarse stand-in for a carpet of the above. A plain opaque cube in every
// respect -- the batch is an albedo, not a shape -- and it exists only at stride 2 and
// coarser, where a one-voxel tuft cannot. Appended, never inserted, same as every id above.
pub const MEADOW: BlockId = 19;
/// Batch 92, roadmap A8 (`--meadow-side`): the meadow with its *side* face repainted too.
/// **Appended, per the ids rule** -- this table's last row was `verdant lamp` since batch 59,
/// now it is this, and `def`'s clamp on an unknown id therefore lands on a grassy proxy in
/// either table generation, which is the whole point the scene's `lamps` comment records.
pub const MEADOW_SIDE: BlockId = 23;
// Batch 59's emitters. Appended, never inserted, same as every id above them. Three
// blocks rather than one because a colour channel demonstrated by a single lamp is a
// channel nobody can see mixing, and mixing is the half `light::flood` had to learn.
pub const AMBER_LAMP: BlockId = 20;
pub const AZURE_LAMP: BlockId = 21;
pub const VERDANT_LAMP: BlockId = 22;
// Batch 101e (`--grass-dense`, a G2 arm). Two new ground-cover ids, appended after
// everything since batch 59 by the rule batch 94b had to learn: the floor of the trouble
// is that an inserted id quietly renames every row that follows it. `GRASS_TALL` is the
// two-cell tall tussock of the plates' 1-2 m stands; `REEDS` is the shoreline stalk that
// closes the references' wet-bank fringe. Both are foliage cross-quads like the tuft they
// descend from, and like it they are *air until a blade is hit* -- see `is_tuft` in
// `common.wgsl`.
pub const GRASS_TALL: BlockId = 24;
pub const REEDS: BlockId = 25;
pub const BLOCK_COUNT: usize = 26;

/// Texture layer indices in the procedural atlas (see `textures.rs`).
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
    /// Batch 92 (roadmap A8): the coarse meadow's side face. Appended to the atlas; the
    /// 24 layers above are byte-identical to every batch since 18.
    pub const MEADOW_SIDE: u32 = 24;
    /// Batch 101e (`--grass-dense`): the tall tussock and the bank reed, appended to the
    /// atlas; the 25 layers above are byte-identical to every batch since 92.
    pub const GRASS_TALL: u32 = 25;
    pub const REEDS: u32 = 26;
    pub const COUNT: u32 = 27;
}

/// How a layer's texture may be permuted per block to break phase alignment (batch 36).
///
/// **The class belongs to the *texture*, not to the block or to the face**, which is the one
/// structural decision in this batch. `GRASS_SIDE` is four faces of one block; binding the
/// class to the layer makes it impossible to give the same picture two different rules, which
/// is the bug a per-face table would eventually have. It is also what OptiFine's shipped
/// `natural.properties` does, and the mode names below are its mode names.
///
/// `face_uv` takes the position *inside* the voxel, so every block samples the identical 0..1
/// of its layer in phase, forever. A hashed permutation of those coordinates is the only fix
/// that stays inside 0..1 -- see `errors.md` for the offset that does not -- and on a
/// 16x16 tile magnified with `Nearest` it is an **exact texel permutation**: no resampling, no
/// fractional phase, and it commutes with `downsample`'s box filter so every mip is the
/// permutation of the mip rather than a blur of it.
pub mod perm {
    /// Identity. The default, and what every directional, bounded or seam-carrying layer
    /// keeps.
    pub const NONE: u32 = 0;
    /// Hashed mirror of u alone. For a layer whose content is bounded in v but arbitrary in
    /// u -- `GRASS_SIDE` is the whole reason this class exists. Its fringe height is a
    /// function of x, identical in every block, so on a slope the grass/dirt boundary lines
    /// up block after block and reads as chevrons. Mirroring u alternates that profile while
    /// leaving the boundary at the same *height*, so the corduroy goes and gravity stays.
    pub const FLIP_U: u32 = 1;
    /// Hashed full dihedral group of the square: eight states from one transpose and two
    /// mirrors. Only for layers whose content is isotropic *and* not torus-periodic.
    pub const D4: u32 = 2;
}

/// The permutation class of every atlas layer, indexed by `tex::` id.
///
/// **The rule that decides an entry is what the generator in `textures.rs` actually draws**,
/// and there are exactly three disqualifiers:
///
/// - *Directional content.* `LOG_SIDE`'s grain runs along x, `PLANKS`' rows run along y,
///   `TALL_GRASS`' blades grow upward. Rotating any of them is a new defect, not variety.
/// - *Bounded content.* `GRASS_SIDE` has a fringe at one end and soil at the other; it takes
///   `FLIP_U` and never `D4`.
/// - *Torus periodicity, which is the one nobody had priced.* `blob()` wraps with
///   `rem_euclid(16)`, so `STONE`, `GRAVEL`, `COBBLE`, `GLOWSTONE`, `BEDROCK`, `SNOW` and
///   half of `LICHEN` tile **seamlessly** across block boundaries today -- a stone wall reads
///   as continuous rock rather than as stacked tiles. A permutation breaks that join. Whether
///   the variety is worth the seam is a look question this batch deliberately does not
///   answer: it leaves them at `NONE`, so nothing that currently tiles seamlessly stops.
pub const PERM_CLASS: [u32; tex::COUNT as usize] = {
    let mut c = [perm::NONE; tex::COUNT as usize];
    // Isotropic per-texel noise (`hash`, not `blob`), so a permutation is free variety.
    c[tex::DIRT as usize] = perm::D4;
    c[tex::GRASS_TOP as usize] = perm::D4;
    c[tex::SAND as usize] = perm::D4;
    c[tex::PODZOL as usize] = perm::D4;
    c[tex::LEAVES as usize] = perm::D4;
    c[tex::PINE_LEAVES as usize] = perm::D4;
    c[tex::MEADOW_TOP as usize] = perm::D4;
    // FLIP_U, `GRASS_SIDE`'s class and the one a banded side tile takes: a mirror keeps the
    // fringe at the top of the tile (a 90-degree turn would put it on its side) and varies
    // only the fringe profile. Batch 94's table, and the `lamps` arithmetic around it, is
    // the same shape of lesson.
    c[tex::MEADOW_SIDE as usize] = perm::FLIP_U;
    // The batch's subject. See `perm::FLIP_U`.
    c[tex::GRASS_SIDE as usize] = perm::FLIP_U;
    c
};

/// `block_faces` entries pack the atlas layer in the low `FACE_LAYER_BITS` and the
/// [`PERM_CLASS`] above it.
///
/// Sixteen bits rather than the five `tex::COUNT` needs, because the free space costs nothing
/// and the next batch that wants N procedural variants per material lands at 168 layers
/// without touching this split.
pub const FACE_LAYER_BITS: u32 = 16;
pub const FACE_LAYER_MASK: u32 = (1 << FACE_LAYER_BITS) - 1;

/// One `block_faces` word: the layer, and the layer's permutation class above it.
pub fn pack_face(layer: u32) -> u32 {
    debug_assert!(layer <= FACE_LAYER_MASK, "atlas layer outruns the packed field");
    layer | (PERM_CLASS[layer as usize] << FACE_LAYER_BITS)
}

// `cross_quad` reads a foliage layer during *traversal* to alpha-test a blade, and it does not
// go through the permutation. A permuted foliage layer would therefore be marched against one
// orientation and shaded with another -- a blade whose silhouette and whose pixels disagree.
// The table above is already `NONE` there; this is what stops a later edit changing it.
const _: () = assert!(PERM_CLASS[tex::TALL_GRASS as usize] == perm::NONE);
// `shade_water` reads the water layer through `face_layer` and deliberately does **not**
// permute, so that a water pixel pays no hash at all. The layer is flat since batch 26, so
// the two agree today for a second reason as well -- but only this one is enforced.
const _: () = assert!(PERM_CLASS[tex::WATER as usize] == perm::NONE);

#[derive(Clone, Copy, Debug)]
pub struct BlockDef {
    pub name: &'static str,
    /// Face order matches the normal ids used by the tracer: -X, +X, -Y, +Y, -Z, +Z.
    pub faces: [u32; 6],
    /// Collides with the player and stops a block pick. **Not** the same as occupying a
    /// voxel: water fills its voxel and the tracer stops on it, but you walk into it.
    pub solid: bool,
    /// Blocks the light flood. Water does not — it attenuates instead, in `light.rs`.
    pub opaque: bool,
    /// Rendered as an in-voxel cross-quad rather than a cube, and **its atlas alpha means
    /// opacity, not the batch-12 tint mask**. That reversal is the whole reason this flag
    /// exists rather than a block-id compare: a tuft wants an alpha cutout *and* a biome
    /// tint out of one texture, and the two meanings cannot share a channel without
    /// something saying which is which. For a foliage block the tint mask is implied 1.
    ///
    /// Independent of `solid` and `opaque` the same way water's three answers are: a tuft
    /// occupies its voxel (the tracer must stop to test it), does not block light, and does
    /// not block the player.
    pub foliage: bool,

    /// Rendered as a 4^3 sub-voxel occupancy mask carved out of its cube, so a canopy has
    /// holes in it and a ray that finds one carries on to whatever stands behind. **The cutout
    /// is geometry and not an alpha test**, which is the whole reason this is a separate flag
    /// from [`Self::foliage`]: a tuft's atlas alpha *is* its opacity and its tint mask is
    /// implied 1, and leaves need both meanings at once. Holes in the tree discard no texel,
    /// so the atlas alpha stays the batch-12 tint mask for every block and leaves keep their
    /// biome colour.
    ///
    /// Independent of `opaque` on purpose: a leaf still stops the light flood, because a
    /// per-block attenuation scalar is a separate question this batch did not open. The *sun*
    /// dapples anyway and for free -- a shadow ray meets the same holes the camera does,
    /// because they are the same geometry.
    pub cutout: bool,

    /// What this block emits, as one flood level per channel, 0..=[`crate::light::MAX_LEVEL`].
    ///
    /// **Three levels rather than one scalar and one colour, because the flood is where two
    /// lamps meet.** A red lamp and a blue one lighting the same corridor have to *mix*, and a
    /// scalar level carrying a per-block tint cannot: the cell between them holds one number
    /// and no memory of which block put it there. Three independent floods hold `(15, 2, 1)`
    /// beside `(1, 2, 15)` and their overlap reads purple, which is the whole of what roadmap
    /// R4 adds and the reason the channel had to widen rather than the table.
    ///
    /// **A level is not a colour and the difference is `light_curve`.** `resolve` renders
    /// `vec3(light_curve(r), light_curve(g), light_curve(b))`, and that curve is
    /// `l^2 (0.6 + 0.4 l)` -- so authoring a hue here means authoring the levels whose *curved*
    /// values carry it, not the levels proportional to it. [`GLOWSTONE`]'s row is the worked
    /// example and the one that had to be exact.
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
    // Batch 38b. `opaque: false` is the *world* half of that batch and the half no shader
    // override can revert: the flood runs through a pane, so a glass house is lit. It is
    // water's answer to the same three questions with the middle one alone changed -- glass
    // still stops the player and still stops a ray, and only the light goes through.
    BlockDef { name: "glass",       faces: all(tex::GLASS),                                 solid: true,  opaque: false, foliage: false, cutout: false, light: [0, 0, 0] },
    // Batch 59. **The retired `BLOCK_TINT` in its 4-bit spelling, and the one row here that
    // had to be derived rather than chosen.** Until this batch `resolve` carried
    // `BLOCK_TINT = (1.0, 0.65, 0.32)` as the colour of *all* block light -- one constant for
    // every emitter in the world, which is exactly the shape of thing this roadmap rung
    // retires. Its colour moves here, into the only block that ever emitted, expressed as the
    // levels whose `light_curve` values come closest to it: `light_curve` of 15, 13 and 9 is
    // (1.0000, 0.7111, 0.3024) against the constant's (1.0, 0.65, 0.32).
    //
    // **Green lands 9.4% high and there is no 4-bit level that does better** -- 12 is 0.5888,
    // 9.4% *low*, and the two bracket the target almost exactly symmetrically. 13 is taken on
    // the tie-break that glowstone's own texture is greener than the constant was
    // (`ALBEDO` normalises to (1.0, 0.794, 0.302)), so the lamp and its light now agree
    // slightly better than they did. What that costs is measured rather than argued:
    // `--no-light-rgb` reads the old scalar back out of `max(r, g, b)` -- which is 15 here and
    // at every cell the flood reaches, because all three channels decrement together -- so the
    // control is bit-exact against the pre-59 build and the shipping frame is what moves.
    BlockDef { name: "glowstone",   faces: all(tex::GLOWSTONE),                             solid: true,  opaque: true,  foliage: false, cutout: false, light: [15, 13, 9] },
    BlockDef { name: "bedrock",     faces: all(tex::BEDROCK),                               solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "water",       faces: all(tex::WATER),                                 solid: false, opaque: false, foliage: false, cutout: false, light: [0, 0, 0] },
    // Batch 9's surfaces.
    BlockDef { name: "snow",        faces: all(tex::SNOW),                                  solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "podzol",      faces: all(tex::PODZOL),                                solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    BlockDef { name: "pine leaves", faces: all(tex::PINE_LEAVES),                           solid: true,  opaque: true,  foliage: false, cutout: true,  light: [0, 0, 0] },
    BlockDef { name: "lichen",      faces: all(tex::LICHEN),                                solid: true,  opaque: true,  foliage: false, cutout: false, light: [0, 0, 0] },
    // Batch 14. Occupies its voxel, blocks neither light nor the player -- water's three
    // answers exactly, with a cross-quad in place of a full cell.
    BlockDef { name: "tall grass",  faces: all(tex::TALL_GRASS),                            solid: false, opaque: false, foliage: true,  cutout: false, light: [0, 0, 0] },
    // Batch 18. Grass's own side and bottom: only the top face is repainted, because only
    // the top face is what a carpet of tufts is seen *through* from height. It is a cube and
    // not foliage -- `foliage: true` here would put a cross-quad at a stride where the
    // marcher's in-voxel intersection means nothing.
    BlockDef { name: "meadow",      faces: column(tex::GRASS_SIDE, tex::DIRT, tex::MEADOW_TOP), solid: true, opaque: true, foliage: false, cutout: false, light: [0, 0, 0] },
    // Batch 59's three emitters, and **the content is the batch** -- roadmap R4's own finding
    // was that `def(..).light > 0` had been true of exactly one block, so a coloured channel
    // would have shipped against an empty world. Each is an ordinary opaque cube: the batch
    // widens a channel and fills the table, and deliberately adds no primitive shape, because
    // the visibility key's `normal:3` has been full since batch 14 and a torch is a shape.
    //
    // **Saturated on purpose, and further apart than a lamp catalogue would put them.** What
    // has to be visible is one lamp's colour arriving on a wall the *other* lamp also lights;
    // three tints of warm white would demonstrate the flood and not the mixing. Blue reaching
    // 15 where red reaches 3 is what makes a stone wall between them read purple.
    BlockDef { name: "amber lamp",  faces: all(tex::AMBER_LAMP),                            solid: true,  opaque: true,  foliage: false, cutout: false, light: [15, 9, 3] },
    BlockDef { name: "azure lamp",  faces: all(tex::AZURE_LAMP),                            solid: true,  opaque: true,  foliage: false, cutout: false, light: [3, 9, 15] },
    BlockDef { name: "verdant lamp",faces: all(tex::VERDANT_LAMP),                          solid: true,  opaque: true,  foliage: false, cutout: false, light: [4, 15, 6] },
    // Batch 92, roadmap A8. Same cube as `meadow`, one difference: the side face is
    // `MEADOW_SIDE` -- a grass-side tile shaded by `MEADOW_SHADE`, so a steep coarse slope
    // shows the carpet's edge rather than LOD 0's bright grass fringe. Stamped only by the
    // `--meadow-side` arm; the unarmed world is the pre-92 world block for block.
    //
    // **The row sits at the end of the table, which is the same rule the id obeys.** Batch
    // 94 pinned this with asserts after the row was first written *ahead of the lamps* --
    // the id said 23, the array said 20, and every batch-59 emitter shifted one id up,
    // which is what 865,982 pixels of `lamps` was. The id and its row are one list kept in
    // two places; the asserts below are what keeps them one list.
    BlockDef { name: "meadow-side", faces: column(tex::MEADOW_SIDE, tex::DIRT, tex::MEADOW_TOP), solid: true, opaque: true, foliage: false, cutout: false, light: [0, 0, 0] },
    // Batch 101e (`--grass-dense`): the two new ground-cover forms. Cross-quads on the
    // tuft's contract -- `foliage` is what resolves the alpha channel as opacity rather
    // than tint mask, and a solid one of these would be a green cube in silence.
    BlockDef { name: "grass, tall", faces: all(tex::GRASS_TALL),                            solid: false, opaque: false, foliage: true,  cutout: false, light: [0, 0, 0] },
    BlockDef { name: "reeds",       faces: all(tex::REEDS),                                 solid: false, opaque: false, foliage: true,  cutout: false, light: [0, 0, 0] },
];

// Batch 94: the id-and-row agreement, pinned at compile time, because the ids are array
// positions with no constants and the failure is silent re-naming, not a crash. `lamps`
// at `MEADOW_SIDE`'s row price is the argument.
const _: () = assert!(BLOCKS[MEADOW_SIDE as usize].faces[0] == tex::MEADOW_SIDE);
// 101e appends the grass pair after it: same id-and-row agreement, and the *test* that
// named `MEADOW_SIDE` the end of the list now names `REEDS` -- the pin's job is to catch
// the shape of the next insertion, not to enshrine one row.
const _: () = assert!(BLOCKS[GRASS_TALL as usize].faces[0] == tex::GRASS_TALL);
const _: () = assert!(BLOCKS[REEDS as usize].faces[0] == tex::REEDS);
const _: () = assert!(BLOCKS[GRASS_TALL as usize].foliage && BLOCKS[REEDS as usize].foliage);
const _: () = assert!(REEDS as usize == BLOCK_COUNT - 1);
// The three lamps ahead of it kept their ids -- each one pinned by the light only it can
// carry, so a future append that shifts the tail fails to compile rather than to shade.
// Element-wise on purpose: array `==` is not const on the shipping toolchain, which is
// what the batch-94 gate's build failure was -- the fork's one-line local patch is here
// upstreamed, because a fix that lives only in the fork dies at the next sync.
const _: () = assert!(
    BLOCKS[20].light[0] == 15 && BLOCKS[20].light[1] == 9 && BLOCKS[20].light[2] == 3
);
const _: () = assert!(
    BLOCKS[21].light[0] == 3 && BLOCKS[21].light[1] == 9 && BLOCKS[21].light[2] == 15
);
const _: () = assert!(
    BLOCKS[22].light[0] == 4 && BLOCKS[22].light[1] == 15 && BLOCKS[22].light[2] == 6
);

/// Mean linear albedo of each block's **+Y face**, for roadmap R2's bounce gather.
///
/// **Why the top face and not a mean over all six.** What reads this is `probe::bake`, asking
/// what colour the light bouncing off the *ground* is, and the ground is seen from above. A
/// block's sides and bottom are not lit by the sky in the first place, so averaging them in
/// would darken grass by its own dirt.
///
/// **Why a `const` table and not a sample of the atlas.** The bake runs on sixteen rayon
/// workers with no GPU and no device, and `build_atlas` takes a run-time argument
/// (`--water-mottle`), so a sampled table would be a second thing that varies with a flag
/// nothing else about the bounce depends on. What keeps it honest is
/// `tests/probe.rs::albedo_matches_the_atlas`, which rebuilds the atlas and asserts every row
/// -- so the table cannot drift from the textures without the suite saying so, which is the
/// one failure mode a hand-written table has.
///
/// Linear, not sRGB: the atlas is `Rgba8UnormSrgb` and everything the compute passes compute
/// is linear radiance, so the mean is taken after decoding and never before. Averaging the
/// bytes instead biases every entry dark, which is the same argument `downsample` makes.
pub const ALBEDO: [[f32; 3]; BLOCK_COUNT] = [
    [0.1738, 0.1738, 0.1738], // air -- never read; stone's value so a bug is not a black hole
    [0.1738, 0.1738, 0.1738], // stone
    [0.2066, 0.1016, 0.0470], // dirt
    [0.0920, 0.2947, 0.0407], // grass
    [0.6170, 0.5245, 0.2573], // sand
    [0.1598, 0.1458, 0.1328], // gravel
    [0.2828, 0.1724, 0.0622], // log
    [0.0389, 0.1941, 0.0226], // leaves
    [0.3439, 0.1974, 0.0648], // planks
    [0.2269, 0.2269, 0.2269], // cobblestone
    [0.4397, 0.6282, 0.7365], // glass
    [0.8058, 0.6399, 0.2432], // glowstone
    [0.0651, 0.0651, 0.0744], // bedrock
    [0.0097, 0.0931, 0.1470], // water
    [0.7240, 0.7812, 0.9029], // snow
    [0.0844, 0.0386, 0.0137], // podzol
    [0.0153, 0.0848, 0.0386], // pine leaves
    [0.1574, 0.1857, 0.1178], // lichen
    [0.0121, 0.0685, 0.0063], // tall grass
    [0.0317, 0.0936, 0.0152], // meadow
    [0.6480, 0.2539, 0.0576], // amber lamp
    [0.1397, 0.2884, 0.6801], // azure lamp
    [0.2356, 0.6124, 0.1454], // verdant lamp
    // Batch 92: top face is `MEADOW_TOP`, the same layer, so the row is meadow's. The
    // `albedo_matches_the_atlas` proof regenerates it either way.
    [0.0317, 0.0936, 0.0152], // meadow-side
    [0.0371, 0.0987, 0.0080], // grass, tall -- 101e: blades' RGB mean over the tile, alpha-independent
    [0.0266, 0.0444, 0.0064], // reeds -- 101e: stalks are narrow, so the mean is mostly zero texels
];

/// Rec.709 luminance, which is what [`ALBEDO`]'s normalisation is taken in.
///
/// Named here rather than spelled out in `probe.rs` because a second copy of these three
/// weights is a second answer waiting to disagree with the first, and the test that pins the
/// table reads them too.
#[inline]
pub fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The bar a player starts with, and the only bar any build had before batch 34.
///
/// **Since batch 34 this is a default rather than the bar itself**: [`crate::player::Player`]
/// holds the live one, a journal can carry one, and `--bar` can author one. What still speaks
/// for this copy alone is the `const` assert beside `render::FLAG_FOLIAGE` -- a compile-time
/// check can only ever check the compile-time array -- which is why the same refusals are
/// restated at run time in [`bar_slot_refusal`] for every bar that arrives from a file or a
/// flag.
pub const HOTBAR: [BlockId; 10] = [
    STONE, DIRT, GRASS, SAND, PLANKS, LOG, LEAVES, COBBLE, GLASS, GLOWSTONE,
];

/// How many slots a bar has. Named rather than spelled `HOTBAR.len()` at each site, because
/// the file format, the `--bar` parser and the HUD all have to agree with it and a
/// `#[repr(C)]` struct cannot say "the length of that array".
pub const SLOTS: usize = HOTBAR.len();

/// What the player has to hand, one block per slot. The live copy is `Player::bar`; the one
/// on disk is the journal's bar block.
pub type Bar = [BlockId; SLOTS];

/// Why this block may not sit in a bar slot, or `None` if it may.
///
/// **Every refusal here keeps an argument another file already makes**, which is also why the
/// list is this short. Until batch 34 the only sources of a block id were the generator and
/// one `const` array, and three places reasoned about that; a bar settable from a file or a
/// flag is a new source, and these are the three arguments it would otherwise quietly break.
///
/// - **`TALL_GRASS` is the hard one.** `render::SPEC_FOLIAGE` compiles the marcher *unable*
///   to see a tuft in a run the generator gave none, so a placed one would be invisible
///   rather than wrong. The `const` assert next to `render::FLAG_FOLIAGE` exists to stop
///   exactly that arriving quietly, and it can only speak for `HOTBAR`.
/// - **`WATER` is the milder one**, and it keeps `scene::flags_from`'s argument true: a
///   zeroed sea level floods nothing, so `FLAG_WAVES` is allowed to assume a run with no sea
///   in it can never acquire a water pixel later.
/// - **`AIR` is not a hazard but a category error.** A slot holds the block you place, and
///   placing air is what breaking a block already is.
///
/// Returned as the reason rather than a `bool` because both callers -- the file's edge and
/// the flag parser -- have to say *why* in a sentence, and a boolean would put that sentence
/// in two places.
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

/// Occupies a voxel, so the tree stores an occupancy bit for it and the tracer stops on
/// it. Everything but air; water included, which is what gives the surface a hit to shade.
#[inline]
pub fn fills_voxel(id: BlockId) -> bool {
    id != AIR
}

/// Flat table of **8** texture layers per block, uploaded to the GPU.
///
/// Six cube faces and the two cross-quad planes batch 14 added, which are normal ids 6 and 7
/// and so have to be addressable at the same stride. The widening is why `resolve` indexes
/// `block_faces[id * 8u + normal_id]` and not `* 6u`: entries 0..5 are byte-identical to what
/// they were, so every existing block renders exactly as before, and the stride became a
/// shift instead of a multiply on the way past.
///
/// A non-foliage block's two extra entries are never read -- nothing can produce normal 6 or
/// 7 for a cube. They are filled with the block's own first face rather than a sentinel so
/// that a bug which *did* read them shows that block's texture on a diagonal, which is
/// obvious in a capture, instead of some unrelated block's, which is not.
/// Entries per block in the table `block_faces` is built from: the six cube faces, then
/// face 0 twice for batch 14's two cross-quad normal states.
///
/// **Named because three WGSL call sites multiply by it and one of them was wrong for eight
/// batches.** `shade_water` was written in batch 8 against the stride of six that held then
/// and was missed when batch 14 widened it, so water's +Y face read index 81 -- block 10
/// face 1, which is `tex::GLASS`. A shader cannot see this constant, so what ties them is the
/// assert beside `block::WATER` in `render/mod.rs` and `faces_per_block_matches_the_shader`
/// in `tests/textures.rs`.
pub const FACES_PER_BLOCK: usize = 8;

pub fn face_table() -> Vec<u32> {
    let mut v = Vec::with_capacity(BLOCK_COUNT * FACES_PER_BLOCK);
    for b in BLOCKS.iter() {
        // Packed since batch 36, so **every WGSL reader has to go through `face_layer`** --
        // the same hazard the stride of eight already carries one line below, and the same
        // remedy: one accessor, and a test that holds every call site to it.
        for f in b.faces {
            v.push(pack_face(f));
        }
        v.push(pack_face(b.faces[0]));
        v.push(pack_face(b.faces[0]));
    }
    v
}

/// Rendered as a cross-quad instead of a cube. See `BlockDef::foliage`.
#[inline]
pub fn is_foliage(id: BlockId) -> bool {
    def(id).foliage
}

/// Transmits rather than occludes: the primary ray stops on it and `resolve` shades it with a
/// refraction leg instead of an albedo. See `render::FLAG_GLASS`.
///
/// **An id compare and not a `BlockDef` flag, which is water's shape and deliberately not
/// [`is_cutout`]'s.** A flag earns its place when more than one block answers yes -- leaves and
/// pine leaves are two, so `cutout` is a field -- and glass is one. `common.wgsl` mirrors the
/// id the same way it mirrors [`WATER`], pinned by the `const` assert beside
/// `render::FLAG_GLASS`.
#[inline]
pub fn is_glass(id: BlockId) -> bool {
    id == GLASS
}

/// Any light level on any channel. Roadmap P11: the truth the emitter-resident gate on
/// `SPEC_EMITTER_GATHER` is computed from, so it lives beside the table it mirrors.
pub fn is_emitter(id: BlockId) -> bool {
    def(id).light != [0, 0, 0]
}

/// Does this block carry a sub-voxel cutout? See [`BlockDef::cutout`].
///
/// The marcher cannot call this -- a shader cannot see this crate -- so `common.wgsl` mirrors
/// the answer as two id compares, and the asserts beside `render::FLAG_LEAF_CUTOUT` are the
/// only thing holding the two copies together.
pub fn is_cutout(id: BlockId) -> bool {
    def(id).cutout
}




/// Artistic sunlight policy, independent of collision and visible opacity.
pub fn casts_sun_shadow(id: BlockId) -> bool {
    def(id).opaque && id != GLOWSTONE
}
