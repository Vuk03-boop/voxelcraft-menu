# Building a Minecraft-like Voxel Engine in Rust

A reference on prior art and unexploited research.

Compiled September 2026. Target language: **Rust**.

**Cut to what this engine uses (batch 28).** Sections 2, 3, 4.3-4.5 and most of 5 are gone: meshing crates, greedy meshing, vertex packing, mesh shaders and a rasteriser build order, none of which apply to an engine with no mesher -- which is the thing `CLAUDE.md`'s opening paragraph exists to warn against. **The section numbers are deliberately unchanged**, because `README.md` and `PERF.md` cite 4.1 and 4.2 by number.

---

## Contents

- **§1** [Prior attempts — what exists and how far it got](#1-prior-attempts)
- **§4** [Frontier work not yet in any shipping voxel game](#4-frontier-work)
- **§6** [Reading list](#6-reading-list)

---

## 1. Prior attempts

### 1a. Permissively licensed — safe to copy code from

| Project | Language | Licence | How far it got |
|---|---|---|---|
| [fogleman/Craft](https://github.com/fogleman/Craft) | C + OpenGL | MIT | Complete playable clone in ~2000 lines. Chunk meshing, terrain gen, client/server multiplayer, block place/break. Small enough to read in an afternoon. |
| [Terasology](https://github.com/MovingBlocks/Terasology) | Java | Apache 2.0 | Large, architecturally serious. Module system, ECS, sophisticated world gen. A real engine, not a demo. |
| [japsuu/Korpi](https://github.com/japsuu/Korpi) | C# / OpenTK | MIT | Mid-size voxel engine with published technical documentation on its implementation — rarer and more valuable than the code. |
| [TanTanDev/binary_greedy_mesher_demo](https://github.com/TanTanDev/binary_greedy_mesher_demo) | **Rust / Bevy** | MIT OR Apache-2.0 | Binary greedy mesher with criterion benchmarks comparing culled vs. greedy meshing. The most directly reusable Rust reference here. |
| [cgerikj/binary-greedy-meshing](https://github.com/cgerikj/binary-greedy-meshing) | C++ | MIT | The canonical binary greedy meshing implementation. v2 is several times faster than v1. Supports 64³ chunks including neighbour data. |
| [Vercidium/voxel-mesh-generation](https://github.com/Vercidium/voxel-mesh-generation) | C# | — | Source for the *Sector's Edge* mesher. Paired with a blog post that gives per-optimisation percentage gains — the single most honest performance writeup in the space. |
| [tyronx/occlusionculling](https://github.com/tyronx/occlusionculling) | C# | — | *Vintage Story*'s production occlusion culling. Raycasting reimplementation of Tommo's algorithm that fixes its under-culling flaw. |
| [dannowilby/mcrs](https://github.com/dannowilby/mcrs) | **Rust / wgpu** | — | Infinite world on all 3 axes, frustum + occlusion culling, procedural 3D noise, player physics. Includes a JS prototype of the chunk occlusion system. Check the licence file before copying. |
| [tim-oster/voxel-rs](https://github.com/tim-oster/voxel-rs) | **Rust / OpenGL** | — | Not a rasteriser: infinite voxel terrain raytraced against a Sparse Voxel Octree, with physics and walkable worlds at 60–120 FPS. Derivative of Laine & Karras. |
| [dubiousconst282/VoxelRT](https://github.com/dubiousconst282/VoxelRT) | C++ / Slang | — | Benchmark harness comparing ESVO, Tree64, BVH+DDA and brickmap backends head to head. The measurements that make the frontier section below trustworthy. |
| [expenses/tree64](https://github.com/expenses/tree64) | **Rust** | — | Rust implementation of the 4³-branching sparse tree structure. |

### 1b. Copyleft or otherwise not copyable — read, don't paste

| Project | Language | Licence | Why it still matters |
|---|---|---|---|
| [Luanti](https://github.com/luanti-org/luanti) (formerly Minetest) | C++ | LGPL-2.1 | The mature one. A voxel *game-creation platform*: you write Lua for items and gameplay, it handles rendering and networking. 62,000³ block worlds, 3100+ mods on ContentDB. **If your goal is a game rather than an engine, build on this in Lua and skip everything below.** v5.17.0 released August 2026. [Site](https://www.luanti.org/) · [Docs](https://docs.luanti.org) |
| [Veloren](https://gitlab.com/veloren/veloren) | **Rust** | GPL-3.0 | The largest Rust voxel codebase in existence. Open-world RPG inspired by Dwarf Fortress and Cube World. Best available reference for Rust chunk streaming and multithreaded meshing architecture. **GPL-3 is viral — reading is fine, pasting relicenses your project.** |
| [pixelguys/cubyz](https://github.com/pixelguys/cubyz) | Zig | — | Interesting for its design discussions rather than code — e.g. its [lighting issue thread](https://github.com/pixelguys/cubyz/issues/61) works through why Minecraft's 16-level, 2-channel flood-fill lighting is limiting and what coloured long-range lighting would cost (3× for colour channels, 8× for doubling range). |

### 1c. Directory

[millennIumAMbiguity/open-source-voxel-game-list](https://github.com/millennIumAMbiguity/open-source-voxel-game-list) — curated list of open-source voxel games with screenshots, feature matrices and FPS figures at standardised settings (1080p, 512-block render distance).

### 1d. Filtering warning

GitHub's `voxel-engine` and `minecraft-clone` topics return hundreds of repos. Most are a weekend of someone's learning and stop at "you can place a block." Filter on:

- **Commit history depth** — under ~50 commits is usually a tutorial follow-along.
- **Does it solve chunk loading without stutter?** This is where toy projects die.
- **Assets folder licence separately from code licence.** Many ship Minecraft's actual textures, which you cannot use regardless of the code licence.

---

## 4. Frontier work

Published, benchmarked, with reference implementations — and essentially absent from shipping voxel games. Ordered by (size of win) × (how unexploited).

### 4.1 Sparse Voxel DAGs — the biggest unexploited idea

**Paper:** [High Resolution Sparse Voxel DAGs](https://icg.gwu.edu/sites/g/files/zaxdzs6126/files/downloads/highResolutionSparseVoxelDAGs.pdf) — Kämpe, Sintorn, Assarsson.

**The insight:** an SVO efficiently encodes *empty* space. Generalising the tree to a directed acyclic graph lets nodes share pointers to identical subtrees, so it also efficiently encodes *identical* space. Minecraft-like worlds are enormously repetitive, which is exactly the case this exploits.

**Measured:** node count reduced by **one to three orders of magnitude** across all tested scenes including highly irregular ones. A scene at 945MB as a DAG required 5.1GB as an SVO, not even counting pointers. AO and shadows raytraced in the DAG at 170 and 240 MRays/sec, with primary shading from conventional rasterisation. The paper gives a bottom-up SVO→minimal-DAG reduction that works even when the full SVO wouldn't fit in memory.

**Why nobody shipped it:** DAGs handle only static data. Sharing subtrees means editing one block invalidates every region that shared that node. Fatal for a game about editing blocks.

**That blocker is now solved, and I can find no voxel game using the solution.**

**HashDAG** — Careil, Billeter & Eisemann, CGF 2020. [Paper](https://www.semanticscholar.org/paper/Interactively-Modifying-Compressed-Sparse-Voxel-Careil-Billeter/aad2d577108b832c7e7bd085f028c65c67aac62d) · **Code: [Phyronnaz/HashDAG](https://github.com/Phyronnaz/HashDAG)**

Embeds the DAG in a hash table where a node's hash is computed from its contents, so identical nodes hash identically and can be found efficiently. Edits build bottom-up: compute the leaf, search for an existing match, reuse it if found, pass upward. Result is interactive modification of compressed voxel geometry with **no decompress/recompress cycle**, plus compressed colour attributes. Demonstrated with carving, filling, copying and painting — including San Miguel voxelised at 64k³, solidified then carved.

**GPU editing backend** — Pacific Graphics 2024. **Code: [mathijs727/GPU-SVDAG-Editing](https://github.com/mathijs727/GPU-SVDAG-Editing)**

Extends HashDAG with GPU-based editing so large edit operations run at real-time frame rates. CUDA + C++20.

**Further refinements:**
- Molenaar & Eisemann (2023) — lossy and lossless methods for interactively compressing *colour* data during editing, replacing HashDAG's naive CPU colour compression.
- [Encoding Occupancy in Memory Location](https://onlinelibrary.wiley.com/doi/10.1111/cgf.70292) (CGF 2025) — encodes structural information into the pointers themselves, avoiding memory accesses entirely for some traversal steps. More compact, faster traversal, faster editing at similar memory. Explicitly demonstrates a HashDAG adaptation.
- van der Laan (2020) — Lossy SVDAGs, trading fidelity for further compression.

**Assessment:** 10–1000× memory reduction, real-time editing, GPU-side edits, published source. This is the clearest research-to-product gap in the entire space. It is also the hardest thing on this list — CUDA reference code, no Rust port, and you'd be porting rather than integrating.

### 4.2 Wide sparse trees — use 4³, not octrees

**Guide with full source: [A guide to fast voxel ray tracing using sparse 64-trees](https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing/)** · **Benchmarks: [dubiousconst282/VoxelRT](https://github.com/dubiousconst282/VoxelRT)** · **Rust: [expenses/tree64](https://github.com/expenses/tree64)**

**Important negative result first:** benchmarking found even state-of-the-art SVO ray traversal performs **worse than much simpler hierarchical grid methods, by as much as 60%**. Octrees are seductive and the basic premise does not hold up. Branching factor 2 makes nodes too small; encoding overhead dominates.

**The fix — 4³ (64-branching) nodes**, where the child population bitmask fits exactly in one `u64`:

```rust
// 12 bytes
struct SvtNode64 {
    is_leaf: bool,      // 1 bit  — packed with child_ptr
    child_ptr: u31,     // 31 bits — absolute offset
    child_mask: u64,    // which children/voxels exist
}
```

**Memory:** ESVO's 4-byte 2³ node gives ~0.57 bytes/voxel. This gives `12/64 + 12/4096 + ... ≈ 0.19` bytes/voxel — **~3× better**, and with *absolute* pointers, which makes construction and edits easier. Real measurement on Bistro voxelised at ~8k: **224.1MB at ~0.62 bytes/voxel** vs. the same scene as ESVO at **368.0MB at ~1.02 bytes/voxel**. Non-solid voxelised models came out ~60% smaller.

**Traversal optimisations, each measured on an integrated GPU at 4K:**

| Technique | Cycles/ray | Gain |
|---|---|---|
| Baseline naive march | 16,903 | — |
| **+ Ancestor memoisation** | 8,896 | **~2×** |
| + Coalescing single-cell skips | 7,052 | 21% |
| + Ray-octant mirroring | 6,358 | 10% |

- **Ancestor memoisation** — keep an 11-entry stack of node *indices* (not payloads; reload from the storage buffer as needed). After advancing, find which ancestor to backtrack to by XORing current and next positions and taking `firstbithigh` of the result masked with `0xFFAAAAAA`. Described by the author as the most impactful change by a wide margin. Group-shared memory for the stack may give a further small boost.
- **Coalescing single-cell skips** — one bitmask test, `(child_mask >> (child_idx & 0b101010) & 0x00330033) == 0`, identifies empty 2³ cuboids essentially for free.
- **Ray-octant mirroring** — flip coordinates into the negative ray octant so the AABB side-plane offset bakes into the coordinate system. XOR the mantissa rather than computing `3.0 - x`, which produces slightly off results near the exclusive upper bound.
- **Fractional coordinates in [1.0, 2.0)** — define each tree as a cube in that range, recursively subdivided by ¼. Because the range spans exactly one float exponent, you can manipulate the mantissa directly in 2-bit chunks to address cells, eliminating integer↔float conversion and rescaling during traversal entirely.

**Honest limits from the author:** anisotropic / unaligned cell sizing barely pays — iteration counts drop only 2–5%. Grazing rays remain a problem, especially near flat planes not aligned to cell boundaries. And **wide nodes have much higher entropy and cannot represent complete leaf paths, so DAG deduplication works far less well** — §4.1 and §4.2 are largely mutually exclusive. Pick one.

**Practical note:** at world scale, prefer many smaller trees under a top-level grid over one giant tree — memory management and streaming become far simpler.

### 4.6 Colored and long-range lighting

Minecraft's lighting is a flood fill with 16 light levels and 2 channels — no colour, and a torch lights at most 16 blocks. Cubyz's [design thread](https://github.com/pixelguys/cubyz/issues/61) works out the cost of fixing this: 3 colour channels triples the work, and doubling light range costs 2³ = 8×. Large caves look wrong under Minecraft's model because torchlight cannot reach the opposite wall.

Nobody has shipped a good solution. If you're using a raytraced backend (§4.1/§4.2), voxel cone tracing through the hierarchy gives you global illumination almost for free — brickmaps have LOD built in, which is exactly what cone tracing needs.

---

## 5. Recommended build order

**Phase 4 — pick a frontier bet.** Either:
- **Conservative:** keep rasterisation, add GPU-side meshing and GPU-driven culling. Large throughput win, existing pipeline preserved. **Not the path taken**, and the sections describing it were cut with the rest of the meshing material.
- **Aggressive:** abandon rasterisation for a 64-tree raytracer (§4.2) with HashDAG storage (§4.1). Largest possible win, hardest path, and you lose every off-the-shelf thing built on triangles — shadow maps, existing shaders, engine integration, and essentially all tutorial material.

---

## 6. Reading list

### Foundational

- [Meshing in a Minecraft game](https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/) — Lysenko, 2012. The original greedy meshing article.
- [Voxel World Optimisations](https://vercidium.com/blog/voxel-world-optimisations/) — Robinson, 2019. Per-optimisation percentages. **Read this one first.**
- [Further Voxel World Optimisations](https://vercidium.com/blog/further-voxel-world-optimisations/) — the follow-up, down to 0.48ms per chunk.
- [The Advanced Cave Culling Algorithm, Part 1](https://tomcc.github.io/2014/08/31/visibility-1.html) and [Part 2](https://tomcc.github.io/2014/08/31/visibility-2.html) — Cavalieri, 2014.
- [A level of detail method for blocky voxels](https://0fps.net/2018/03/03/a-level-of-detail-method-for-blocky-voxels/) — Lysenko, 2018.
- [Minecraft Wiki: Culling](https://minecraft.wiki/w/Culling) — what vanilla actually does.

### Raytracing and data structures

- [A guide to fast voxel ray tracing using sparse 64-trees](https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing/) — 2024. The best modern practical guide, with measurements.
- [Efficient Sparse Voxel Octrees](https://research.nvidia.com/publication/2010-02_efficient-sparse-voxel-octrees-analysis-extensions-and-implementation) — Laine & Karras, NVIDIA 2010. The reference SVO paper. Note the negative benchmark result in §4.2 before building on it.
- [High Resolution Sparse Voxel DAGs](https://icg.gwu.edu/sites/g/files/zaxdzs6126/files/downloads/highResolutionSparseVoxelDAGs.pdf) — Kämpe et al.
- [Interactively Modifying Compressed Sparse Voxel Representations](https://www.semanticscholar.org/paper/Interactively-Modifying-Compressed-Sparse-Voxel-Careil-Billeter/aad2d577108b832c7e7bd085f028c65c67aac62d) — Careil, Billeter, Eisemann, CGF 2020. HashDAG.
- [Encoding Occupancy in Memory Location](https://onlinelibrary.wiley.com/doi/10.1111/cgf.70292) — Modisett et al., CGF 2025.
- [Fast Voxel Data Structures](https://bink.eu.org/fast-voxel-datastructures/) — accessible overview of grids, octrees, brickmaps and voxel SDFs, and when each wins.
- [Ray Tracing with Voxels in C++](https://jacco.ompf2.com/2024/04/24/ray-tracing-with-voxels-in-c-series-part-1/) — Bikker's series.

### GPU-driven rendering

- [vkguide: GPU Driven Rendering Overview](https://vkguide.dev/docs/gpudriven/gpu_driven_engines/)
- [Towards Practical Meshlet Compression](https://arxiv.org/pdf/2404.06359) — 2024.
- [Using Turing Mesh Shaders: NVIDIA Asteroids Demo](https://developer.nvidia.com/blog/using-turing-mesh-shaders-nvidia-asteroids-demo/)

### Community discussion

- [HN: Binary Greedy Voxel Meshing Algorithm](https://news.ycombinator.com/item?id=40087213) — the pushback on the popular demo. Short and worth reading before you trust any voxel benchmark.

---

## Sanity checks

1. **Benchmark against your own terrain.** Greedy meshing goes from 1.1ms to 36.8ms between smooth terrain and a checkerboard. Someone else's sphere benchmark tells you nothing about your world gen.

2. **Count your block types.** Several fast meshers were benchmarked with 3, and at least one scaled by duplicating chunk data per block type — fine at 3, impossible at 500.

3. **Destructibility changes everything.** If terrain changes constantly, cheap meshing beats low triangle counts. Vercidium's run-based mesher exists precisely because greedy meshing couldn't keep up with 16 players destroying things.

4. **Profile before optimising the clever thing.** 22% of one celebrated mesher's runtime was allocating a vec. The bit tricks are not usually where the time is.

5. **Check asset licences separately from code licences.** Many open-source clones ship Minecraft's actual textures.



