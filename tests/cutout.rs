use voxelcraft::block;
use voxelcraft::render;

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

#[test]
fn micro_grid_fits_the_free_bits() {
    let micro = shader_const("MICRO");
    let shift = shader_const("MICRO_SHIFT");
    let mask = shader_const("VOXEL_MASK");
    assert_eq!(micro, 4, "the micro grid is 4^3");
    assert_eq!(micro * micro * micro, 64, "one native u64 of cells");

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

    for v in [[0u32, 0, 0], [63, 63, 63], [12, 5, 40]] {
        assert_eq!(pack(v, [0, 0, 0]), (v[0] << 16) | (v[1] << 8) | v[2]);
    }
    assert!(micro - 1 == mask >> (shift - 2), "the two fields abut");
}

#[test]
fn hit_t_does_not_branch_on_the_cutout_override() {
    let src = render::shader_source();
    let at = src.find("fn hit_t(").expect("hit_t exists");
    let body = &src[at..];
    let end = body.find("\nfn ").expect("another function follows hit_t");
    let body = &body[..end];

    let code: String = body
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("
");
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

#[test]
fn the_voxel_field_is_never_unpacked_with_a_byte_mask() {
    let src = render::shader_source();
    let sites: Vec<_> = src.match_indices("(low >> 16u)").collect();
    // exp-glass-ssr adds a fourth compliant site: the SSR step decodes the
    // visibility payload the same VOXEL_MASK way to shade its sample.
    assert_eq!(sites.len(), 4, "resolve and taa and the water_sec donor and the ssr vis probe");
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

    assert_eq!(
        src.matches("(low >> 22u) & 3u").count(),
        3,
        "all three decoders read the micro coordinate"
    );
}

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

    assert!(
        !src.contains("const LEAF_FILL"),
        "a second copy of the fill fraction is still in the module"
    );
}

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
