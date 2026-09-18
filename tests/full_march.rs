//! Batch 72, roadmap P10: a root-FULL chunk marches as if its tree existed.
//!
//! **The claims here are the ones the acceptance protocol cannot make.** The protocol --
//! `--no-full-march` bit-exact under `implies`, `validate` against `--reference` -- says what
//! the fixed frame is *worth*; it cannot say the fix is wired up. `--no-full-march` reverts
//! the whole branch, so a sweep is bit-exact whatever the marcher does -- the control cannot
//! see inside the loop it switches off. What these tests hold is the state a mixed full
//! chunk actually reaches the shader in, and the two formulas the fix says are one formula.
//!
//! **What is deliberately not here.** That `--no-full-march` reproduces the pre-batch-72
//! frame is not a test: it is the control's whole job, and it holds by construction -- with
//! the override false the branch folds to the lines that were there before, which is the
//! claim the ledger row makes rather than these. A `validate` MAE on the fixed frame is a
//! screenshot; `docs/water.md` carries those numbers when hardware runs them.

use glam::IVec3;
use voxelcraft::block;
use voxelcraft::config::parse_from;
use voxelcraft::voxel::tree::dense_index;
use voxelcraft::voxel::{ChunkKey, World, VOL};

/// The world state the batch exists for: a 64^3 region wholly under the sea surface and
/// holding the floor -- water above, stone below, no air. `build_local` has no other course
/// but to mark every 16^3 cell full, and `make_root` has no other course but to collapse
/// the run. **Read the three asserts as one claim**: P10's defect is not a hypothetical; it
/// is the joint state `is_full && has_water && dry_mask not all set`, and if the builder
/// ever stops reaching it, the roadmap's 31,815-pixel measurement has nothing to hang on.
#[test]
fn a_mixed_chunk_of_sea_and_floor_reaches_the_shader_as_a_full_mixed_root() {
    let mut dense = vec![block::AIR; VOL];
    for y in 0..64 {
        for z in 0..64 {
            for x in 0..64 {
                dense[dense_index(x, y, z)] = if y < 32 { block::STONE } else { block::WATER };
            }
        }
    }
    let t = voxelcraft::voxel::build_local(&dense);
    let mut world = World::new();
    let key = ChunkKey::new(0, IVec3::new(0, 0, 0));
    world.insert_local(key, t);
    let rec = &world.chunks[&key];

    assert!(
        rec.root.is_full(),
        "water above stone, every voxel occupied: the root has to collapse"
    );
    assert!(rec.has_water, "and the chunk has to say so");
    assert_ne!(
        rec.dry_mask, u64::MAX,
        "the water-only cells have to stay out of the dry mask"
    );
    assert_ne!(
        rec.dry_mask, 0,
        "and the floor's cells have to be in it: with the fix the whole march runs on this word"
    );
    assert!(
        rec.dry_mask.count_ones() == 32,
        "the bottom 32 of the 64 16^3 cells hold dry voxels, the top 32 do not"
    );
}

/// **The two places the shape `vec4<u32>(MAX, MAX, FULL_BIT, l1b * 64)` appears are one fact,
/// not two.** `leaf_attr` has answered full chunks from a dense attribute run since the tree
/// was written; batch 72's march arm synthesizes the same node from the same premise. If
/// either half drifts -- one side goes to 128 entries per cell, say -- the marcher reads a
/// neighbouring leaf's block id and the water test is wrong by four voxels, which is the
/// class of wrong nothing in the frame reports. Anchoring both sides on the literal keeps
/// them moving together or failing here.
///
/// **The anchor stops before the closing paren, and that is deliberate since batch 74.**
/// P9's waterline rode into the march-arm copy (`| 0xF0000000u`, nibble 15 = "no line",
/// because a synthesized node has no tree to have measured one from) while `leaf_attr`'s
/// readers mask the nibble off (`& 0x0FFFFFFFu`) -- so the two sites now differ in their
/// tail, by design, and the shared prefix is the whole of the claim this test ever had.
#[test]
fn the_marcher_and_leaf_attr_synthesize_the_same_full_root_node() {
    let src = voxelcraft::render::shader_source();
    let synth = "vec4<u32>(0xFFFFFFFFu, 0xFFFFFFFFu, FULL_BIT, l1b * 64u";
    let count = src.matches(synth).count();
    assert!(
        count >= 2,
        "the synthesized full-root L1 node appears {count} time(s); leaf_attr and the march loop must both have it"
    );
}

/// The control keeps the pre-batch-72 path bit-exact, and the gate has to be the override.
/// Batch 38's measured rule is what this pins: the constant has to reach the branch that
/// returns, not merely the variable it defaults -- `root_full && !SPEC_FULL_MARCH` written
/// as a folded `select` or buried one level in has left machine code on this pass before.
#[test]
fn the_override_reaches_the_return() {
    let src = voxelcraft::render::shader_source();
    let override_decl = "override SPEC_FULL_MARCH: bool = true;";
    assert!(
        src.contains(override_decl),
        "SPEC_FULL_MARCH is not declared as an override in common.wgsl"
    );
    assert!(
        src.contains("root_full && !SPEC_FULL_MARCH"),
        "the control path has to test the override at the return itself"
    );
}

/// `--no-full-march` flips the config and nothing else: the semantic that keys
/// `FLAG_HI_FULL_MARCH` starts at the pair having made it out of the command line whole.
/// (The flag's membership of `SPEC_HI_MASK` is a `const _: () = assert!` in `render`, which
/// fails the build rather than a test run.)
#[test]
fn the_control_parses() {
    assert!(
        voxelcraft::config::Config::default().full_march,
        "the fix ships on"
    );
    let args: Vec<String> = vec!["--no-full-march".to_string()];
    let (cfg, _mode, unknown) = parse_from(&args);
    assert!(
        unknown.is_empty(),
        "--no-full-march parsed as unknown: {unknown:?}"
    );
    assert!(!cfg.full_march, "--no-full-march has to turn the fix off");
}

