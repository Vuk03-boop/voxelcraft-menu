use criterion::{criterion_group, criterion_main, Criterion};
use voxelcraft::block::*;
use voxelcraft::voxel::*;

fn hills(seed: u32) -> Vec<BlockId> {
    let mut d = vec![AIR; VOL];
    for z in 0..64usize {
        for x in 0..64usize {
            let fx = (x as f32 + seed as f32 * 13.0) * 0.11;
            let fz = (z as f32 + seed as f32 * 7.0) * 0.09;
            let h = (30.0 + 10.0 * fx.sin() * fz.cos() + 4.0 * (fx * 3.1).cos()) as usize;
            for y in 0..h.min(64) {
                let id = if y + 1 == h {
                    GRASS
                } else if y + 4 >= h {
                    DIRT
                } else {
                    STONE
                };
                d[dense_index(x, y, z)] = id;
            }
        }
    }
    d
}

fn bench_build(c: &mut Criterion) {
    let dense = hills(1);
    c.bench_function("build_local hills", |b| b.iter(|| build_local(&dense)));
    c.bench_function("insert_local hills", |b| {
        b.iter_batched(
            || build_local(&dense),
            |t| {
                let mut w = World::new();
                w.insert_local(ChunkKey::new(0, glam::IVec3::ZERO), t);
                w
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_build);
criterion_main!(benches);
