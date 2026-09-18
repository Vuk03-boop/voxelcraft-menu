//! Batch 38's leaf cutout: the six free bits, and the two shapes that cost a frame.
//!
//! The shader is the only place the micro-cell is packed and unpacked, so most of what is
//! worth checking here is a claim *about the shader source* rather than about a Rust function.
//! That is the same job `faces_per_block_matches_the_shader` and `face_layer_at_every_call_site`
//! do for `block_faces`, and for the same reason: a wrong mask returns a *plausible*
//! coordinate rather than garbage.

use voxelcraft::block;
use voxelcraft::render;

/// The layout constants, read off the shader rather than retyped here.
fn shader_const(name: &str) -> u32 {
    let src = render::shader_source();
    let pat = format!("const {name}: u32 = ");
    let at = src
        .find(&pat)
        .unwrap_or_else(|| panic!("{name} is not declared in the shader"));
    let rest = &src[at + pat.len()..];
    let end = rest.find('u').expect("a u32 literal");
    rest[..end].parse().expect("a u32 literal")
}

/// 4^3 is 64 cells in one `u64`, and 2 bits an axis is exactly the six bits the voxel field
/// has had spare since the key was designed. **Both halves matter**: a 8^3 mask would need 9
/// bits and there is no ninth, and a 2^3 one would leave four bits unspent and read as damaged
/// masonry rather than foliage.
#[test]
fn micro_grid_fits_the_free_bits() {
    let micro = shader_const("MICRO");
    let shift = shader_const("MICRO_SHIFT");
    let mask = shader_const("VOXEL_MASK");
    assert_eq!(micro, 4, "the micro grid is 4^3");
    assert_eq!(micro * micro * micro, 64, "one native u64 of cells");
    // 64^3 needs 6 bits an axis; the field gives 8; the micro coordinate rides above.
    assert_eq!(mask, 63, "a chunk axis is 6 bits");
    assert_eq!(shift, 6, "the micro coordinate starts where the voxel one ends");
    let micro_bits = 32 - (micro - 1).leading_zeros();
    assert_eq!(micro_bits, 2, "2 bits an axis");
    assert_eq!(
        (shift + micro_bits) * 3,
        24,
        "three bytes exactly: the voxel field does not grow"
    );
}

/// The packing is a bijection, and it leaves the voxel coordinate alone.
///
/// A transliteration of `make_key`'s low word and the decode both call sites do, which is
/// affordable here only because it is checked against the shader's own constants above rather
/// than against numbers typed twice.
#[test]
fn micro_round_trips_without_disturbing_the_voxel() {
    let shift = shader_const("MICRO_SHIFT");
    let mask = shader_const("VOXEL_MASK");
    let micro = shader_const("MICRO");
    let pack = |v: [u32; 3], m: [u32; 3]| {
        let b = [v[0] | (m[0] << shift), v[1] | (m[1] << shift), v[2] | (m[2] << shift)];
        (b[0] << 16) | (b[1] << 8) | b[2]
    };
    let unpack = |low: u32| {
        let b = [(low >> 16) & 0xFF, (low >> 8) & 0xFF, low & 0xFF];
        ([b[0] & mask, b[1] & mask, b[2] & mask], [b[0] >> shift, b[1] >> shift, b[2] >> shift])
    };
    for v in [[0, 0, 0], [63, 63, 63], [1, 62, 33], [63, 0, 7]] {
        for m in [[0, 0, 0], [3, 3, 3], [1, 2, 3], [0, 3, 1]] {
            let (gv, gm) = unpack(pack(v, m));
            assert_eq!(gv, v, "voxel survived the micro packing");
            assert_eq!(gm, m, "micro survived it");
        }
    }
    // And a zero micro is the pre-batch-38 word unchanged, which is what lets `--no-leaf-cutout`
    // be bit-exact rather than merely close.
    for v in [[0u32, 0, 0], [63, 63, 63], [12, 5, 40]] {
        assert_eq!(pack(v, [0, 0, 0]), (v[0] << 16) | (v[1] << 8) | v[2]);
    }
    assert!(micro - 1 == mask >> (shift - 2), "the two fields abut");
}

/// **`hit_t` must not branch on `SPEC_LEAF_CUTOUT`, and this test is the only thing that says
/// so.** Nine builds established it: the widened decode is free, the extra parameter is free,
/// and an `if SPEC_LEAF_CUTOUT` inside this function -- as an early return, as a selected
/// offset, or hoisted into its own dispatch function -- makes the driver rebuild the *folded*
/// build one ULP away from what it built before, for 201 pixels of 230,400 at a max channel
/// delta of 1 against `voxelcraft-pre38.exe`. The plane is computed unconditionally in micro
/// units instead, which reduces to the cube's own face exactly.
///
/// Deleting this test costs nothing today and costs a bit-exact control the next time someone
/// tidies that function.
#[test]
fn hit_t_does_not_branch_on_the_cutout_override() {
    let src = render::shader_source();
    let at = src.find("fn hit_t(").expect("hit_t exists");
    let body = &src[at..];
    let end = body.find("\nfn ").expect("another function follows hit_t");
    let body = &body[..end];
    // Comments stripped first: the explanation of *why* there is no branch here names the
    // override, and a test that its own documentation can fail is not checking the code.
    let code: String = body
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("SPEC_LEAF_CUTOUT"),
        "hit_t branches on SPEC_LEAF_CUTOUT -- that costs the control its bit-exactness; \
         see the comment at the plane computation"
    );
    assert!(
        code.contains("f32(MICRO)"),
        "hit_t should compute the plane in micro units unconditionally"
    );
}

/// Every place the shader unpacks the voxel field has to mask to six bits.
///
/// There were two such places -- `resolve` and `taa` -- one open-coded copy each of the
/// same four lines, correct for exactly as long as the top two bits of every byte stayed
/// zero. Batch 81 (P12) added a third: the `water_sec` donor cannot call `hitKind` (it
/// has no lane to call it on) and decodes the voxel word itself. The invariant is
/// unchanged, only its census grew: every site masks, because a `& 0xFFu` reads a
/// coordinate of up to 255 out of a 64-wide chunk -- a *plausible* voxel, one quarter of
/// the time the right one, and never an error.
#[test]
fn the_voxel_field_is_never_unpacked_with_a_byte_mask() {
    let src = render::shader_source();
    let sites: Vec<_> = src.match_indices("(low >> 16u)").collect();
    assert_eq!(sites.len(), 3, "resolve and taa and the water_sec donor");
    for (at, _) in sites {
        let line_end = src[at..].find('\n').map(|e| at + e).unwrap_or(src.len());
        let line = &src[at..line_end];
        assert!(
            line.contains("VOXEL_MASK"),
            "a voxel-field decode masks with something other than VOXEL_MASK: {line}"
        );
        assert!(
            !line.contains("0xFFu"),
            "a voxel-field decode still uses a byte mask: {line}"
        );
    }
    // And each decoder pulls the micro coordinate out beside it -- the census grew with
    // the donor for the same reason the masked one did.
    assert_eq!(
        src.matches("(low >> 22u) & 3u").count(),
        3,
        "all three decoders read the micro coordinate"
    );
}

/// The flag reaches the blocks it is supposed to and no others.
///
/// `cutout` is deliberately independent of `foliage`: a tuft's atlas alpha *is* its opacity,
/// a leaf's is its tint mask, and the two meanings cannot share a channel. Putting both flags
/// on one block would ask the marcher for a cross-quad carved into a sub-voxel grid, which is
/// not a thing.
#[test]
fn only_the_two_leaf_blocks_are_carved() {
    let carved: Vec<&str> = block::BLOCKS
        .iter()
        .filter(|b| b.cutout)
        .map(|b| b.name)
        .collect();
    assert_eq!(carved, vec!["leaves", "pine leaves"]);
    for b in block::BLOCKS.iter() {
        assert!(
            !(b.cutout && b.foliage),
            "{} is both a cross-quad and a carved cube",
            b.name
        );
        if b.cutout {
            assert!(b.opaque, "{}: the light flood is not this batch", b.name);
        }
    }
}

/// The shader tells a carved block from a whole one with two id compares, because it cannot
/// call [`block::is_cutout`]. This is the other half of the `const _: () = assert!` pair beside
/// `render::FLAG_LEAF_CUTOUT`: those pin the ids, this pins that nothing *else* drifted into
/// the shader's list.
#[test]
fn the_shader_knows_exactly_the_carved_ids() {
    let src = render::shader_source();
    assert_eq!(
        shader_const("LEAVES_ID"),
        block::LEAVES as u32,
        "the shader's LEAVES_ID"
    );
    assert_eq!(
        shader_const("PINE_LEAVES_ID"),
        block::PINE_LEAVES as u32,
        "the shader's PINE_LEAVES_ID"
    );
    let carved = block::BLOCKS.iter().filter(|b| b.cutout).count();
    assert_eq!(
        src.matches("id == LEAVES_ID || id == PINE_LEAVES_ID").count(),
        1,
        "is_cutout_id is the one place the shader lists them"
    );
    assert_eq!(carved, 2, "a third carved block needs a line in is_cutout_id");
}

/// The control is in `SPEC_MASK`, which is what makes it free rather than merely available.
#[test]
fn the_control_is_folded_and_not_branched() {
    assert_ne!(
        render::SPEC_MASK & render::FLAG_LEAF_CUTOUT,
        0,
        "--no-leaf-cutout has to fold, not branch: the marcher is reachable from `resolve` \
         through water's secondary rays, and batch 14 paid +1.85 ms for exactly that"
    );
    let src = render::shader_source();
    assert!(src.contains("override SPEC_LEAF_CUTOUT: bool = true;"));
}

// ------------------------------------------------- batch 43: the leaf fill, swept and pinned

/// The fill fraction is one number written in two files, and this is the only thing tying them.
///
/// `render::LEAF_FILL` is what `make_spec` hands the pipeline; the `override` in `common.wgsl`
/// is what a reader of the shader sees, and what any pipeline built without the constants list
/// would get. They have to agree or the file lies about the shipping canopy -- the same
/// arrangement `the_water_shadow_budget_agrees_with_the_shader` makes for batch 41's budget,
/// and the reason both are worth having is that a *look* constant has no test that can catch
/// it being wrong. Only a picture can, and a picture is not run by `cargo test`.
#[test]
fn the_leaf_fill_agrees_with_the_shader() {
    let src = render::shader_source();
    let key = "override SPEC_LEAF_FILL: f32 = ";
    let at = src
        .find(key)
        .expect("SPEC_LEAF_FILL is not in the module any more");
    let open = at + key.len();
    let close = open + src[open..].find(';').expect("unterminated override");
    let wgsl: f32 = src[open..close]
        .trim()
        .parse()
        .expect("SPEC_LEAF_FILL is not a number");
    assert_eq!(
        wgsl,
        render::LEAF_FILL,
        "the shader's default fill and the one `make_spec` passes disagree"
    );
    // The `const` this replaced was read by name from `leaf_solid` from batch 38 to batch 42.
    // A stale copy left behind would compile, and would quietly be the fraction the carve used.
    assert!(
        !src.contains("const LEAF_FILL"),
        "a second copy of the fill fraction is still in the module"
    );
}

/// `leaf_solid` is the fraction's only reader, and the carve is its only caller.
///
/// The claim worth pinning is not the arithmetic -- that is one comparison -- but that the
/// number reaches the hash in *one* place. A second reader would be a second canopy density
/// that `--no-leaf-thin` does not restore, and no capture would say so: the control would come
/// back bit-exact at every vantage whose trees happened to go through the reader it does cover.
#[test]
fn the_leaf_fill_has_one_reader() {
    let src = render::shader_source();
    let code: String = src
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ");
    let uses = code.matches("SPEC_LEAF_FILL").count();
    assert_eq!(
        uses, 2,
        "SPEC_LEAF_FILL should appear exactly twice in the module -- its `override` and the \
         one comparison in `leaf_solid` -- and it appears {uses} times"
    );
    let at = src.find("fn leaf_solid(").expect("leaf_solid exists");
    let body = &src[at..];
    let end = body.find("\nfn ").expect("another function follows leaf_solid");
    assert!(
        body[..end].contains("< SPEC_LEAF_FILL"),
        "the reader is not the threshold in leaf_solid any more"
    );
}



