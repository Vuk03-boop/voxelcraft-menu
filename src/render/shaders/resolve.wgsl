// Surface repair helpers. Legacy CLI controls are visual comparisons, not old-binary replicas.
fn surface_sky_fill(n: vec3<f32>) -> vec3<f32> {
    let sky = sky_ambient_tint(n);
    if !SPEC_LIGHTING_REPAIR { return sky; }
    // Only sky fill is reduced. Coloured block light never passes through this factor.
    return sky * (0.60 * mix(0.35, 1.0, clamp(n.y * 0.5 + 0.5, 0.0, 1.0)));
}
fn surface_light(ambient: vec3<f32>, sun: vec3<f32>, ao: f32, floor_light: vec3<f32>, face: f32) -> vec3<f32> {
    if !SPEC_LIGHTING_REPAIR { return ((ambient + sun) * ao + floor_light) * face; }
    // Sun-facing contrast comes from N.L, not an axis-dependent material multiplier.
    // AO is an indirect-light approximation, not another sun-shadow test.
    return ambient * ao * face + sun + floor_light;
}
fn surface_radiance(albedo: vec3<f32>, light: vec3<f32>, spec: vec3<f32>, id: u32) -> vec3<f32> {
    if SPEC_LIGHTING_REPAIR && id == GLOWSTONE_ID {
        return albedo * (vec3<f32>(1.0, 0.7111, 0.3024) * 1.5 + light * 0.15);
    }
    return albedo * light + spec;
}

// Pass 5: shade from the visibility buffer. Shadow rays and ambient occlusion are paid
// per visible pixel, never per traced pixel.
//
// Everything here is linear radiance: the atlas is sRGB so `albedo` decodes on fetch, and
// `out_tex` is rgba16float, so the blit owns the tone map and the display encode. The
// scalar light factors — `face_shade`, `light_curve`, the 0.50/0.52 weights — carried over
// from the gamma-space version untouched, on purpose: read as linear they are finally the
// ratios they always claimed to be (a 2:1 top-to-bottom face split, not the 2^2.2 = 4.6:1
// one gamma space was silently applying). The colours did have to change, because they
// were authored as display values and are decoded below.

// **The base batch 57 modulates rather than replaces.** In the pre-57 build this table is the
// whole of directional occlusion: a wall at the foot of a cliff reads exactly like the same
// wall in an open field. The cube supplies the part that depends on where the surface actually
// is, and this keeps the two the cube has no better answer for -- the X/Z asymmetry, which is
// a readability convention rather than anything about where light comes from, and the open-sky
// calibration the cube's occlusion is normalised against. See `docs/shading.md`, and roadmap
// R2 and R5 for what retires them.
//
// **There is no second copy of these numbers in Rust and that is deliberate.** The bake stores
// occlusion alone, so the table is read in exactly one place and cannot drift from itself.
fn face_shade(normal_id: u32) -> f32 {
    switch normal_id {
        case 3u: { return 1.00; }   // +Y top
        case 2u: { return 0.50; }   // -Y bottom
        case 4u, 5u: { return 0.80; } // Z sides
        // Batch 14's cross-quads. A blade is not a wall and does not sit square to one
        // axis, so neither side value is right for it; near the top of the range because a
        // tuft is lit from every direction the two planes between them face.
        case 6u, 7u: { return 0.90; }
        default: { return 0.62; }   // X sides
    }
}

// ---- batch 59: composing sky against *coloured* block light ----
//
// **The pre-59 expression with one substitution, and naming the substitution is the batch.**
// It was `mix(SKY_TINT, BLOCK_TINT, bk / (sk + bk)) * max(sk, bk)`: whichever source dominates
// decides the hue, the brighter one decides the level, and `BLOCK_TINT` was the hue of block
// light everywhere in the world. `blk` now arrives as three channels of radiance, so the hue
// is `blk / max(blk)` -- what actually reached this cell -- and `max(blk)` is the level the old
// scalar held.
//
// **`max` over the channels and not luminance**, for two reasons that agree. It is the value a
// white emitter's single pre-59 flood held, which is what keeps `--no-light-rgb` a revert
// rather than a rescale; and it is the only reduction that leaves the chroma inside the unit
// cube, so a saturated lamp reads at the brightness its strongest channel earned rather than
// being dimmed for being saturated.
//
// The `1e-4` floors are the pre-59 ones and do the same two jobs: an unlit cell divides by the
// larger of nothing and epsilon, and `mix` at weight zero returns `SKY_TINT` exactly, so a
// world with no emitter in it is bit-identical to one compiled without this at all.
// **`sky` is `SKY_TINT` or what batch 63 derived in its place, and it is passed rather than
// read.** The hue of the sky half is a property of the *face* since roadmap R7 -- a floor and a
// wall see different parts of the gradient -- so this function cannot look it up, and every
// caller computes it once per `shade_hit` and hands it down. With `SPEC_SKY_TINT` cleared every
// caller passes the constant and this is the pre-63 expression character for character.
fn block_ambient(blk: vec3<f32>, sk: f32, sky: vec3<f32>) -> vec3<f32> {
    let bk = max(blk.r, max(blk.g, blk.b));
    let tint = blk / max(bk, 1e-4);
    return mix(sky, tint, bk / max(sk + bk, 1e-4)) * max(sk, bk);
}

// Debug palette, authored as display colours; decoded so the heatmap reads the same now
// that the blit re-encodes on the way out.
fn heat(v: f32) -> vec3<f32> {
    let t = clamp(v, 0.0, 1.0);
    let c = vec3<f32>(
        smoothstep(0.35, 0.85, t),
        smoothstep(0.0, 0.45, t) - smoothstep(0.65, 1.0, t),
        1.0 - smoothstep(0.0, 0.45, t),
    );
    return pow(c, vec3<f32>(2.2));
}

// Minecraft-like response: light level 15 is full, and it falls off steeply. In linear
// space this tracks Minecraft's own 0.8^(15-level) closely (0.23 vs 0.21 at level 8).
fn light_curve(level: f32) -> f32 {
    let l = level / 15.0;
    return l * l * (0.6 + 0.4 * l);
}

// The same curve over three channels, for batch 59's block light. Componentwise and nothing
// else -- the curve is per channel by construction, because a level is a flood distance and
// the three floods are independent.
fn light_curve3(level: vec3<f32>) -> vec3<f32> {
    let l = level / 15.0;
    return l * l * (vec3<f32>(0.6) + 0.4 * l);
}

// Cool sky against warm block light. Both were authored as display colours and are the
// decoded values; in linear they sum to near-neutral where the two sources overlap.
//
// **Batch 63 kept this and changed what it is.** Until roadmap R7 it was the colour of the sky
// everywhere in the world -- one authored constant multiplied into `amb` on every shaded pixel,
// scaled only by `frame.daylight`, with the sky model the renderer actually draws sitting
// unconsulted in the next file. It is now the *calibration* of a derived hue rather than the
// hue, which is exactly what `face_shade` became at batch 57 and `floor_rgb` at batch 58: the
// value the derivation is defined to return at one reference condition, named below.
const SKY_TINT: vec3<f32> = vec3<f32>(0.6038, 0.7084, 1.0000);

// ---- batch 63, roadmap R7: the colour of the sky this face can actually see ----
//
// **Rec.709, and the same three weights `block::luma` carries.** A second copy of them is a
// second answer waiting to disagree with the first; that function's own comment says so, and
// this is the shader-side copy it was warning about. `probe.rs` normalises `BOUNCE_REF` in this
// luminance and so does the tint below, which is what makes the two rungs commensurable.
fn luma3(c: vec3<f32>) -> f32 {
    return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
}

// **The cosine-weighted mean of the gradient's own elevation parameter over the part of a
// face's hemisphere that is above the horizon, renormalised over that same set.** Derived by
// quadrature, not authored, and the two numbers are the whole of the directional half.
//
// `n.y` is exactly +1, 0 or -1 for every normal state this renderer has -- a cube face's normal
// is axis aligned and `normal_of` returns `(0.707, 0, +/-0.707)` for both cross-quad planes --
// so three values cover every face and a `select` is exact where a fitted curve would not be.
//
// **The up case has a closed form and the quadrature reproduces it, which is what says the
// quadrature is right.** For a normal facing straight up the whole hemisphere is sky and the
// integral is `2 * integral of u^(1+SKY_GRAD_EXP) du` over [0,1], or `2 / (2 + SKY_GRAD_EXP)` =
// 2/2.55. The side case has no such form and converges to about 0.58578 -- 0.58584712 at 300
// polar steps, 0.58580391 at 600, 0.58578915 at 1200 -- and the fourth decimal of a lerp
// parameter is not visible in any frame.
//
// **A down-facing face sees no sky at all**, so its hemisphere has no sky to average and the
// weight is the horizon: it is the colour of the lowest sky there is, which is the right
// stand-in for whatever reaches the underside of an overhang. Very little rides on it -- the
// flood's own `sk` at such a cell is near zero and multiplies everything here.
const SKY_W_UP: f32 = 0.78431373;
const SKY_W_SIDE: f32 = 0.58578900;

// The gradient a face pointing straight up sees at full daylight, which is R7's reference
// condition: `mix(SKY_DAY_HORIZON, SKY_DAY_ZENITH, SKY_W_UP)`. Spelled as a literal because
// WGSL has no const `mix`, and pinned against that expression by
// `tests/sky_tint.rs::the_reference_is_the_gradient_it_says_it_is` so the two cannot drift.
const SKY_AMBIENT_REF: vec3<f32> = vec3<f32>(0.14658824, 0.30178431, 0.84105882);

// `luma3(SKY_TINT)`, spelled out for the same reason and pinned by the same test.
const SKY_TINT_LUMA: f32 = 0.70721556;

// **The sun's halo counted as energy rather than sampled as a direction.** `scatter_lobe` is
// normalised so isotropic scattering reads 1.0, so the lobe integrates over the whole sphere to
// `4 * pi`; a lobe that narrow, averaged over a cosine-weighted hemisphere, is that energy times
// the cosine weight at the sun, and the `1 / pi` of the average leaves `4`. So this is derived
// from `hg_gain`'s own normalisation and is not a tuning knob -- there is no authored constant
// anywhere in this batch.
//
// **Sampling `hg_gain` along one representative direction was the alternative and is worse.**
// The lobe is about 30 degrees wide at the shipping `--fog-g 0.80` and the hemisphere is not, so
// a single sample reads the wrong side of a cliff whenever the sun sits off that direction but
// still in view -- which at a sunset is most of the frame. The cosine falls off smoothly and has
// no such edge.
const SKY_HALO_ENERGY: f32 = 4.0;

// **How far the derived hue is taken from `SKY_TINT`, and the one authored number in this
// batch.** It is an exponent on the per-channel ratio, so 1.0 is the physical answer, 0.0 is
// `SKY_TINT` exactly, and above 1.0 exaggerates the same hue rather than inventing a different
// one -- the luminance renormalisation below is applied after it, so **every rung of this ladder
// is exposure-neutral** and R7's trap stays disarmed at any strength.
//
// **It exists because the physical answer is very nearly invisible, which the user found with an
// image slider and the fixture then confirmed**: at 1.0 the `default` pair is **MAE 1.2185**,
// against the **2.87** at which [`ADR 0001`](../../../docs/adr/0001-no-elevation-form-factor.md)
// rejected a change by eye and the 0.372 batch 60 called invisible on its own. Two reasons
// compound. The luminance renormalisation deliberately removes the *brightness* difference,
// which is the larger half of what separates a real sky from this constant; and a
// hemisphere-averaged sky is far less saturated than the zenith people picture when they say
// "the sky is blue", so what is left over is a modest rotation -- diluted again by `amb * 0.50`
// standing beside a direct sun term.
//
// **So the physical rung is a real measurement and a poor feature, and this is the honest way to
// say both.** The ladder is swept by eye and one value ships, which is `LEAF_FILL`'s shape at
// batch 43 exactly -- and for `LEAF_FILL`'s reason, that no metric in this tree can rank a look
// and the user is the instrument. `docs/shading.md` carries the sweep.
const SKY_TINT_SAT: f32 = 2.5;

// The hue of the sky a face with this normal can see, at `SKY_TINT`'s own luminance.
//
// **Two identities, and both are exact rather than close.** At the reference condition -- a
// face pointing straight up, full daylight, and no halo, which `--fog-scatter 0` reaches -- `c`
// is `SKY_AMBIENT_REF`, the ratio is 1 on every channel and this returns `SKY_TINT` to the bit.
// And for every other condition `luma3` of the result is `SKY_TINT_LUMA` by construction, so
// **only the hue moves and never the exposure**. That second identity is what makes this batch
// rankable: roadmap R7's own trap is that the real sky is far darker than the constant standing
// in for it -- day zenith is (0.0637, 0.2140, 0.8276) against (0.6038, 0.7084, 1.0000) -- so a
// build that substituted radiance naively would darken every shadow in the world and no
// before/after pair could separate that from the feature arriving. Batch 57 lost a batch to
// exactly that ambiguity; `probe::BOUNCE_REF` is the same rule one rung earlier.
//
// So what ships is a *departure from* `SKY_TINT`, not a replacement of it.
fn sky_ambient_tint(n: vec3<f32>) -> vec3<f32> {
    if !SPEC_SKY_TINT {
        return SKY_TINT;
    }
    let day = frame.daylight;
    // The same four endpoints and the same lerp `sky_base` takes, from the one copy in
    // `common.wgsl`. What differs is the parameter: a view ray has an elevation and a face has
    // a hemisphere, so where the sky raises `clamp(rd.y, 0, 1)` to `SKY_GRAD_EXP`, this takes
    // the cosine-weighted mean of that same power over the sky the face can see.
    let horizon = mix(SKY_NIGHT_HORIZON, SKY_DAY_HORIZON, day);
    let zenith = mix(SKY_NIGHT_ZENITH, SKY_DAY_ZENITH, day);
    let w = select(select(SKY_W_SIDE, SKY_W_UP, n.y > 0.5), 0.0, n.y < -0.5);
    var c = mix(horizon, zenith, w);
    // The scattered halo around the sun, which is where a sunset's colour actually lives.
    // **The gradient alone has none of it**: at low sun `frame.daylight` falls, so both
    // endpoints lerp toward the *night* constants and the sky the gradient describes goes dark
    // blue rather than orange. `SCATTER_TINT` is the warm one, `scatter_lobe` is the term that
    // carries it, and until this batch nothing but the sky itself and the haze ever read it.
    //
    // Both this and the gradient above carry `frame.daylight`, so their *ratio* does not --
    // which is what keeps a low sun's warmth from fading out along with the light that causes
    // it. `--fog-scatter 0` zeroes this term alone and leaves the gradient half standing, which
    // is how the two halves of this batch are ranked apart without spending a second flag bit.
    c = c + SCATTER_TINT
        * (frame.fog_scatter * day * SKY_HALO_ENERGY * max(dot(n, frame.sun_dir), 0.0));
    // Per channel against the reference, then back to the reference luminance. The scalar
    // `luma3(SKY_AMBIENT_REF) / luma3(c)` that the first step would want cancels against the
    // second, so it is not written: scaling `t` uniformly cannot change what the last line
    // returns.
    let t = SKY_TINT * pow(c / SKY_AMBIENT_REF, vec3<f32>(SKY_TINT_SAT));
    return t * (SKY_TINT_LUMA / max(luma3(t), 1e-6));
}
// **Batch 59 retired this and it is kept only for `--no-light-rgb`.** Until roadmap R4 it was
// the colour of *all* block light in the world: one authored constant standing in for every
// emitter, which is the shape of thing the main sequence exists to replace. Emission is now a
// per-block row in `block::BLOCKS`, carried through the flood as three levels, and the tint
// `shade_hit` mixes toward is derived from what actually arrived at the cell. `GLOWSTONE`'s row
// is this constant's own value in the 4-bit spelling the flood can carry -- `light_curve` of
// 15, 13 and 9, which is (1.0000, 0.7111, 0.3024) against the (1.0, 0.65, 0.32) below.
//
// **So this is the one retired constant in the ladder that did not survive as a calibration**,
// where `face_shade` and `floor_rgb` both did. It could have: multiplying it by a normalised
// chroma keeps a white emitter bit-exact. What that costs is the gamut -- every channel of the
// product is capped by this constant, so a blue lamp can never be more than 0.32 as bright as
// a white one, and the cap is a property of a colour nobody would author twice. See
// `docs/shading.md`.
const BLOCK_TINT: vec3<f32> = vec3<f32>(1.0000, 0.6500, 0.3200);
// Darkening per occluding neighbour of a face corner, so a corner with three keeps exactly
// a quarter of its light — Minecraft's own vertex step, read here as linear radiance. It is
// up from the 0.21 batch 2 inherited from gamma space, but it is not the 0.113 that would
// reproduce the old *screen* value: bilerping spreads each corner's darkening over a
// quarter of the face where the nearest-corner version confined it to the rim, so the same
// number reads heavier, and the peak is now reached over area rather than at a seam.
const AO_STRENGTH: f32 = 0.25;

// ---- batch 55: the probe tap, which prices roadmap L1's read side and shades nothing ----
//
// **One probe per 4 blocks, which is the dense end of the design and therefore the honest one
// to measure**: a coarser field costs strictly less, because more pixels share a texel. 128
// texels at that spacing span 512 blocks, about the view distance, and `fract` wraps rather
// than the sampler -- `linear_samp` is the history sampler and clamps, so an unwrapped tap
// would send every distant pixel to one edge texel and read artificially cheap.
//
// **`PROBE_TAPS` is 1 and that is a floor rather than a design.** One tap is a DC-only RGB
// field, which is the cheapest thing L1 could store. An SH L1 or a 6-axis ambient cube is
// three, and three is one literal away from here -- batch 55 measures both and quotes both,
// because the number that decides L1's shape is the one for the format L1 would actually use.
// L5 (roadmap): 4-block probes held at the same 512-block XZ period the 8-block field
// wrapped at, so nothing in `common.wgsl`'s toroidal-tap argument re-derives -- the two
// numbers below are mirrored from `PROBE_DIM_XZ` / `probe::SPACING` by hand, the way
// `WATER_ID` is, and the Rust side pins both with `const _` asserts.
const PROBE_DIM: f32 = 128.0;
const PROBE_SPACING: f32 = 4.0;
const PROBE_INV_SPAN: f32 = 1.0 / (PROBE_DIM * PROBE_SPACING);
const PROBE_TAPS: u32 = 1u;
// Enough to land in a different texel each time, so N taps read N addresses the way N
// coefficient textures would, rather than N hits on one cache line.
const PROBE_TAP_STRIDE: f32 = 0.37;

// ---- batch 57: the ambient cube, which is what the tap above was pricing ----
//
// **What it returns is occlusion, not shading**: 1 is an open field and `shade_hit` multiplies
// `face_shade` by it. That is what makes every absence of the feature exact rather than close
// -- an unbaked texel, a coarse LOD and a pinned `--probe-fill 1.0` all reduce to
// `face_shade(id) * 1.0`. The first build of this batch baked the product and `cave` moved
// 367,222 pixels at max delta 1, because 0.62 and 0.80 do not survive `f16` and 1.0 does.
//
// **Six slabs stacked along the texture's Y, one per `normal_id`, and a face reads only its
// own.** That is what makes a six-entry directional basis cost the one tap batch 55 measured:
// picking the slab is a switch on `normal_id`, which `face_shade` already is and which this
// replaces, and the sample itself is one hardware trilinear fetch.
//
// **Where a probe lives.** Probe `i` along an axis is centred on world `PROBE_SPACING * i +
// PROBE_SPACING / 2`, so a texel centre -- which the sampler puts at `i + 0.5` -- is at world
// `p / PROBE_SPACING`. That is the whole address, and it is why `PROBE_INV_SPAN` is the same
// expression batch 55 used. **Writing down where a cell was sampled and where the shader
// thinks it lives before the first capture is `docs/lessons.md`'s rule**, and batch 35 is why:
// it filled each texel from a cell's *centre* and read it back as though the index sat at the
// cell's *edge*, a uniform two-block displacement of every shadow that was invisible in the
// A/B, in the control and in every metric the fixture has.
//
// X and Z wrap in the sampler; Y is clamped **inside the slab** and not by the sampler,
// because the sampler's clamp is the whole texture's edge and would let the top of one slab
// filter into the bottom of the next. `PROBE_DIM - 0.5` is the last texel centre.
const PROBE_SLABS: f32 = 6.0;
// The cross-quads have no axis to name a slab with, so a blade reads the +Y slab and keeps
// `face_shade`'s own 0.90 as its base -- which needs no constant here at all, because the
// multiply in `shade_hit` already applies it. An open field reproduces 0.90 exactly and a tuft
// in a shaded valley dims with the ground it stands on rather than floating brighter than it.
// The roadmap said to decide this by eye and it is the same shrug the constant always was.
// Half a probe along the surface normal, and it is the difference between the feature working
// and the feature being half washed out.
//
// **A surface sits on the boundary its own occluder defines.** The face of a cliff is at the
// plane where the height field steps, so the probe nearest an unbiased `hit` is as likely to be
// *inside* the terrain -- where the cube has no sky and falls back to `face_shade` -- as it is
// to be in the air the face looks out into. Stepping half a probe along the normal moves the
// trilinear weight onto the open side, which is the same normal bias every probe-field renderer
// applies and for the same reason. Half a probe rather than a block because the lattice is
// 4 blocks coarse: a one-block nudge leaves three quarters of the weight where it was.
const PROBE_NORMAL_BIAS: f32 = PROBE_SPACING * 0.5;

// ---- batch 58: the ground bounce, which shares this tap rather than adding one ----
//
// **`.a` is batch 57's sky occlusion and `.rgb` is the bounce's per-channel multiplier on
// `floor_rgb`.** One `textureSampleLevel` returns both, so a directional *coloured* field
// costs exactly what the directional scalar one cost -- which is the finding batch 55 went
// looking for when it measured three taps at the price of one, and the reason `make_probe`
// chose `Rgba16Float` before there was anything to put in the other channels.
//
// Both halves are identity at 1.0, so an unbaked texel returns `vec4(1.0)` and the caller
// multiplies two things by one. That is why this returns the raw `vec4` rather than two
// functions taking the sample twice: a second call is a second tap the driver has no reason
// to fold, and the whole claim of this batch is that the read side did not change.
fn probe_field(p: vec3<f32>, normal_id: u32) -> vec4<f32> {
    let cross = normal_id >= NORMAL_CROSS_A;
    let slab = select(normal_id, 3u, cross);

    let t = p * (1.0 / PROBE_SPACING);
    let y = clamp(t.y, 0.5, PROBE_DIM - 0.5);
    let uvw = vec3<f32>(
        t.x * (1.0 / PROBE_DIM),
        (f32(slab) * PROBE_DIM + y) * (1.0 / (PROBE_DIM * PROBE_SLABS)),
        t.z * (1.0 / PROBE_DIM),
    );
    return textureSampleLevel(probe_tex, probe_samp, uvw, 0.0);
}

fn probe_tap(p: vec3<f32>) -> vec3<f32> {
    var acc = vec3<f32>(1.0);
    for (var i = 0u; i < PROBE_TAPS; i = i + 1u) {
        let uvw = fract(p * PROBE_INV_SPAN + vec3<f32>(0.0, 0.0, f32(i) * PROBE_TAP_STRIDE));
        acc = acc * textureSampleLevel(probe_tex, linear_samp, uvw, 0.0).rgb;
    }
    return acc;
}

// Texture coordinates of a point on a voxel face. Split out so `shade_water` picks exactly
// the texel `shade_hit` would.
fn face_uv(normal_id: u32, local: vec3<f32>) -> vec2<f32> {
    if normal_id >= NORMAL_CROSS_A {
        // Both cross-quads run u along local x. On plane A local z tracks x and on plane B
        // it mirrors it, so one expression covers the pair and the blade texture is not
        // stretched differently on the two halves of the same tuft. v matches the cube
        // convention below so a blade's root is at the bottom of the texture like a wall's.
        return vec2<f32>(local.x, 1.0 - local.y);
    }
    let axis = normal_id >> 1u;
    var uv: vec2<f32>;
    if axis == 0u {
        uv = vec2<f32>(local.z, 1.0 - local.y);
    } else if axis == 1u {
        uv = vec2<f32>(local.x, local.z);
    } else {
        uv = vec2<f32>(local.x, 1.0 - local.y);
    }
    if normal_id == 1u || normal_id == 4u {
        uv.x = 1.0 - uv.x;
    }
    return uv;
}

// The key a block's permutation is drawn from: the integer world coordinate of the voxel's
// **minimum corner**, and the face it was hit on.
//
// `box_min` is `c.origin + vec3<f32>(v) * c.voxel_size` -- built from a chunk origin and an
// integer voxel index, so it is already exact and integral and the `floor` is only here to
// make that claim in the code. **Nothing about a ray reaches this**: not the hit point, not
// the jitter, not the view direction. That is the whole requirement. A key drawn from the
// float hit position would oscillate between two blocks along a face seam as `taa` moved the
// sample, the surface would change texture every frame, and the 3x3 neighbourhood clip would
// read a stable world as a moving one and clamp the history away.
//
// The face is in the key so the six sides of one block draw independently -- they are
// different layers anyway, so the only thing a shared key would buy is a correlation nobody
// can see.
fn perm_key(box_min: vec3<f32>, normal_id: u32) -> u32 {
    let c = vec3<i32>(floor(box_min));
    return hash_u32(bitcast<u32>(c.x) * 0x9e3779b9u
        ^ bitcast<u32>(c.y) * 0x85ebca6bu
        ^ bitcast<u32>(c.z) * 0xc2b2ae35u
        ^ normal_id * 0x27d4eb2fu);
}

// One layer's texture coordinates, permuted into one of its class's states.
//
// **Exact on this atlas and not in general.** The tiles are 16x16 and the sampler magnifies
// with `Nearest`, so a mirror or a transpose is a bijection of the texel grid onto itself:
// no resampling, no fractional phase, no intermediate colour that was not already in the
// palette. It survives minification for a second reason -- a dihedral map sends each 2x2 box
// of `downsample` onto another 2x2 box and only permutes the four inside it, and a sum does
// not care about the order, so the permutation of the mip *is* the mip of the permutation at
// every level.
//
// Written with `select` rather than branches on purpose. `pclass` varies pixel to pixel
// wherever two materials meet, so a branchy version would serialise the wave across both
// paths for the whole 8x8 tile; this way every lane runs the identical arithmetic on
// different registers and nothing diverges.
fn permute_uv(uv: vec2<f32>, pclass: u32, h: u32) -> vec2<f32> {
    let d4 = pclass == PERM_D4;
    // One transpose and two mirrors generate all eight elements of the dihedral group, and
    // the eight combinations of three bits map onto them one to one. `FLIP_U` takes the u
    // mirror alone, which is the one state that leaves v -- and therefore gravity -- fixed.
    var p = select(uv, vec2<f32>(uv.y, uv.x), d4 && (h & 1u) != 0u);
    p.x = select(p.x, 1.0 - p.x, pclass != PERM_NONE && (h & 2u) != 0u);
    p.y = select(p.y, 1.0 - p.y, d4 && (h & 4u) != 0u);
    return p;
}

// ---------------------------------------------------------------------------------------
// The biome field, a second time.
// ---------------------------------------------------------------------------------------
//
// This is `src/biome.rs` in WGSL, evaluated per pixel from the world XZ of the shaded
// point. It **stores nothing**: no per-voxel attribute, no bit of the visibility key (which
// is full), nothing for a coarse LOD to resample and nothing for an edit to relight. The
// price of that is a second implementation of the field, and the rest of this comment is
// about what keeps the two honest.
//
// What the two must agree on is *where a biome is*. They do not have to agree bit for bit,
// and the reason is the shape of what the answer feeds: the palette snaps at `BIOME_BAND`,
// but the tint blends across `BIOME_BLEND` either side of it, so there is no threshold here
// for a last-bit difference to fall the wrong side of. A boundary a few blocks out is a
// blend a few blocks out, which is invisible; a *scrambled* field is not, which is the only
// failure a transliteration typo actually produces.
//
// The chain is: `fastnoise-lite` <- `biome::simplex2`, pinned bit-exactly by
// `noise_matches_fastnoise_lite` over half a million columns and three frequency regimes;
// `biome::simplex2` <- `biome_noise` below, line for line, with `simplex_grad` the one
// deliberate departure. `BIOME_BAND`, `BIOME_BLEND` and the two seed offsets are mirrored
// by hand and pinned by a `const _: () = assert!` in `render/mod.rs`; the two frequencies
// are not mirrored at all and arrive through `Frame`.

const BIOME_BAND: f32 = 0.22;
const BIOME_BLEND: f32 = 0.18;
// `biome::TEMP_SEED` and `biome::HUMID_SEED`.
const BIOME_TEMP_SEED: i32 = 7;
const BIOME_HUMID_SEED: i32 = 8;
// `fastnoise-lite`'s own primes.
const BIOME_PRIME_X: u32 = 501125321u;
const BIOME_PRIME_Y: u32 = 1136930381u;

// `GRADIENTS_2D` is 128 direction pairs: 24 at 15 degrees apart repeated five times, then 8
// at 45 degrees apart. **This is the one place the port is not a transliteration.** The Rust
// twin reads the published decimals out of a 48-entry table, and indexing a table that size
// with a runtime value costs a per-lookup stack copy in a shader, so the same directions are
// rebuilt from the angles they are. It is worth about 1e-7 on a noise value, which is spent
// entirely inside a blend.
const GRAD24_FIRST: f32 = 1.4398966328953218;  // 82.5 degrees, then clockwise
const GRAD24_STEP: f32 = 0.26179938779914946;  // 15 degrees
const GRAD8_FIRST: f32 = 1.1780972450961724;   // 67.5 degrees
const GRAD8_STEP: f32 = 0.7853981633974483;    // 45 degrees

// The hashing is done in u32 because WGSL only *guarantees* wrap-on-overflow for unsigned,
// where the Rust twin says `wrapping_mul` for the same reason. The single shift that has to
// be arithmetic is taken on the bitcast i32, exactly as `h ^= h >> 15` does there.
fn simplex_grad(seed: u32, x_primed: u32, y_primed: u32, xd: f32, yd: f32) -> f32 {
    let h = (seed ^ x_primed ^ y_primed) * 0x27d4eb2du;
    let hs = bitcast<i32>(h);
    // Even, and 0..=254: the published table is indexed in pairs and `| 1` takes the second
    // of one, which here is the sine rather than a neighbouring entry.
    let idx = u32((hs ^ (hs >> 15u)) & 254i);
    var angle: f32;
    if idx < 240u {
        angle = GRAD24_FIRST - GRAD24_STEP * f32((idx % 48u) >> 1u);
    } else {
        angle = GRAD8_FIRST - GRAD8_STEP * f32((idx - 240u) >> 1u);
    }
    return xd * cos(angle) + yd * sin(angle);
}

// Truncation toward zero, then a step down for negatives -- which is *not* `floor`: at an
// exactly-integral negative coordinate this returns one less than `floor` does. That is
// `fastnoise-lite`'s own behaviour and the Rust twin copies it, so this does too.
fn biome_fast_floor(f: f32) -> i32 {
    if f >= 0.0 {
        return i32(f);
    }
    return i32(f) - 1;
}

// One octave of OpenSimplex2 at `freq`, in -1..1. The three corner terms are summed in the
// order `biome::simplex2` sums them, because f32 addition does not associate.
fn biome_noise(seed_i: i32, freq: f32, p: vec2<f32>) -> f32 {
    let seed = bitcast<u32>(seed_i);
    let sqrt3 = 1.7320508075688772;
    let g2 = (3.0 - sqrt3) / 6.0;

    // The skew into simplex space, which `fastnoise-lite` does before dispatching on noise
    // type and so reads as a separate step there too.
    let f2 = 0.5 * (sqrt3 - 1.0);
    var xy = p * freq;
    let s = (xy.x + xy.y) * f2;
    xy = xy + vec2<f32>(s, s);

    let i0 = biome_fast_floor(xy.x);
    let j0 = biome_fast_floor(xy.y);
    let xi = xy.x - f32(i0);
    let yi = xy.y - f32(j0);

    let t = (xi + yi) * g2;
    let x0 = xi - t;
    let y0 = yi - t;

    let i = bitcast<u32>(i0) * BIOME_PRIME_X;
    let j = bitcast<u32>(j0) * BIOME_PRIME_Y;

    let a = 0.5 - x0 * x0 - y0 * y0;
    var n0 = 0.0;
    if a > 0.0 {
        n0 = (a * a) * (a * a) * simplex_grad(seed, i, j, x0, y0);
    }

    let c = (2.0 * (1.0 - 2.0 * g2) * (1.0 / g2 - 2.0)) * t
        + ((-2.0 * (1.0 - 2.0 * g2) * (1.0 - 2.0 * g2)) + a);
    var n2 = 0.0;
    if c > 0.0 {
        let x2 = x0 + (2.0 * g2 - 1.0);
        let y2 = y0 + (2.0 * g2 - 1.0);
        n2 = (c * c) * (c * c)
            * simplex_grad(seed, i + BIOME_PRIME_X, j + BIOME_PRIME_Y, x2, y2);
    }

    var n1 = 0.0;
    if y0 > x0 {
        let x1 = x0 + g2;
        let y1 = y0 + (g2 - 1.0);
        let b = 0.5 - x1 * x1 - y1 * y1;
        if b > 0.0 {
            n1 = (b * b) * (b * b) * simplex_grad(seed, i, j + BIOME_PRIME_Y, x1, y1);
        }
    } else {
        let x1 = x0 + (g2 - 1.0);
        let y1 = y0 + g2;
        let b = 0.5 - x1 * x1 - y1 * y1;
        if b > 0.0 {
            n1 = (b * b) * (b * b) * simplex_grad(seed, i + BIOME_PRIME_X, j, x1, y1);
        }
    }

    return (n0 + n1 + n2) * 99.83685446303647;
}

// `biome::band_weights`: the three band weights, summing to 1, exactly 0.5/0.5 where the
// palette snaps. WGSL's `smoothstep` is the same cubic the Rust one spells out.
fn biome_band_weights(n: f32) -> vec3<f32> {
    let hot = smoothstep(BIOME_BAND - BIOME_BLEND, BIOME_BAND + BIOME_BLEND, n);
    let cold = 1.0 - smoothstep(-BIOME_BAND - BIOME_BLEND, -BIOME_BAND + BIOME_BLEND, n);
    return vec3<f32>(cold, 1.0 - cold - hot, hot);
}

// The six multipliers on albedo. **This is the only place they are written down** -- there
// is no Rust copy, because nothing on that side of the wire has an opinion about them, and a
// mirrored constant with one reader is a drift risk bought for nothing.
//
// Plains is exactly (1, 1, 1) by construction, not by taste: it makes the tint a *departure*
// from the atlas rather than a wash over it, so `--no-tint` and the tinted build agree
// exactly in the middle of the temperature-humidity square and differ more the further out
// a column sits. It is also what keeps the A/B legible -- a diff against the control shows
// the biomes, not a global multiply.
//
// **These are linear ratios, and that is why they do not look like the numbers you would
// pick.** `albedo` here is what the hardware handed back from an sRGB atlas, so it is
// already linear, and a multiplier applied to it reads on screen at roughly its 1/2.2
// power: the 2.81 on tundra's blue is a 1.6x shift to the eye, not a 2.8x one. Each was
// authored as the ratio it should *look* like and raised to 2.2 -- which is also the whole
// reason this is a multiply on albedo and not a `pow` or a display-referred constant
// anywhere near `resolve`, the thing batch 2 exists to keep out.
//
// The shape of the set follows Minecraft's own grass palette rather than being invented:
// what separates its biomes is mostly red and blue, with green nearly fixed. Savanna and
// desert take red up until the green goes to straw; taiga and tundra take blue up until it
// goes cold; forest takes red *down*, which is the only way to get deeper than a green that
// is already the base.
const TINT_PLAINS:  vec3<f32> = vec3<f32>(1.000, 1.000, 1.000);
// Batch 92, roadmap A8's palette arm (--tint-balance, SPEC_TINT_BALANCE): the plains
// *departing* -- the thing the identity row exists to prevent ever shipping silently, so
// it lives behind its own bit rather than as a retune of the row above. Apparent
// (0.97, 1.03, 0.95): a grass-green lean, because a plains that reads as its own biome
// is warm green and nothing else. The other five rows already say something; the arm's
// *second* half is the atlas masks on sand and lichen reaching them, and that half is
// content in textures.rs, not to be found in this file.
const TINT_PLAINS_BALANCED: vec3<f32> = vec3<f32>(0.935, 1.067, 0.892);
const TINT_FOREST:  vec3<f32> = vec3<f32>(0.664, 1.045, 1.000);  // 0.83, 1.02, 1.00 apparent
const TINT_SAVANNA: vec3<f32> = vec3<f32>(1.842, 0.935, 0.793);  // 1.32, 0.97, 0.90
const TINT_DESERT:  vec3<f32> = vec3<f32>(2.096, 0.957, 0.699);  // 1.40, 0.98, 0.85
const TINT_TAIGA:   vec3<f32> = vec3<f32>(0.832, 0.935, 2.096);  // 0.92, 0.97, 1.40
const TINT_TUNDRA:  vec3<f32> = vec3<f32>(0.755, 0.893, 2.812);  // 0.88, 0.95, 1.60

// The tint at a world XZ. `biome::GRID` is `[humidity][temperature]`, and it is written out
// row by row here rather than indexed -- the nine ids it stands for are spelled out in a
// const assert in `render/mod.rs`, which is what pins this layout to that one.
fn biome_tint(world_xz: vec2<f32>) -> vec3<f32> {
    let temperature = biome_noise(frame.tint_seed + BIOME_TEMP_SEED, frame.tint_freq_t, world_xz);
    let humidity = biome_noise(frame.tint_seed + BIOME_HUMID_SEED, frame.tint_freq_h, world_xz);
    let wt = biome_band_weights(temperature);  // cold, mid, hot
    let wh = biome_band_weights(humidity);     // dry, mid, wet
    // The swap is a select on a compile-time bool: off, naga folds it to TINT_PLAINS
    // and this function is the module batch 12 shipped; on, every plains cell in the
    // field takes the balanced row instead. One selection, two readers -- plains is the
    // middle of the temperature square's dry row and of its mid row.
    let plains = select(TINT_PLAINS, TINT_PLAINS_BALANCED, SPEC_TINT_BALANCE);
    let dry = wt.x * TINT_TUNDRA + wt.y * plains + wt.z * TINT_DESERT;
    let mid = wt.x * TINT_TAIGA  + wt.y * plains + wt.z * TINT_SAVANNA;
    let wet = wt.x * TINT_TAIGA  + wt.y * TINT_FOREST + wt.z * TINT_FOREST;
    return wh.x * dry + wh.y * mid + wh.z * wet;
}

// Linear radiance leaving one voxel face toward `rd`, before any medium between it and the
// eye. This was the body of `resolve`; batch 8 lifted it out because the refracted and the
// reflected rays need the *identical* shading. A second surface shaded by a cheaper model
// reads as a different material seen through glass, which is exactly what water is not.
//
// `dist` is the total distance travelled from the eye, and only the texture footprint reads
// it -- so a surface seen in a reflection picks the mip its apparent size deserves rather
// than the one its distance from the mirror would give it.
// `smooth_light` is the batch-45 parameter and it is a *literal at every call site*, which is
// the whole of why it costs nothing. The primary hit passes `true` and its inlined copy folds
// back to the code that was here before; the three secondary hits pass the override, so their
// copies fold to one light lookup. Nothing branches at run time on a per-pixel value.
// How much of the Fresnel-weighted sky reflection to keep. **Authored, and 1.0 is the physical
// answer** -- `schlick_ground` is already the full reflectance, so this is a look control and not
// a fudge factor, in the shape `LEAF_FILL` and `SKY_TINT_SAT` are in. It exists because the
// standing judgement on this world is that it reads too dark, and a term that only ever *adds*
// light is one whose strength somebody should get to choose by eye rather than inherit.
const SKY_SPEC_GAIN: f32 = 0.5;

// A/B control retained verbatim apart from its function name. Do not tune it
// alongside the experimental schedule: it is the reference for shader comparisons.
fn shade_hit_legacy(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool) -> vec3<f32> {
        let c = chunks[ci];
        let vs = c.voxel_size;
        // Batch 14. A cross-quad is double-sided -- there is one blade and the eye may be
        // on either side of it -- so only *which plane* is stored and the facing is settled
        // here against the view ray. That is what makes two normal states enough where a
        // one-sided quad would have needed four.
        let foliage = normal_id >= NORMAL_CROSS_A;
        var n = normal_of(normal_id);
        if foliage && dot(n, rd) > 0.0 {
            n = -n;
        }
        // ---- batch 67, roadmap P8/P13: the specular's direction half, hoisted ----
        //
        // **This is here rather than beside the `spec_rgb` it feeds, and the reason is register
        // allocation rather than clarity.** Written at the end of this function -- next to the
        // return, where it reads naturally -- the term cost `resolve` **16 registers**, taking
        // it from 80 to 96, and that occupancy step is worth 0.156 ms at `cave` where the term
        // changes *zero pixels*.
        //
        // **It is not any one of the three operations.** Batch 67 stubbed them one at a time and
        // every simplification read 80: dropping `sky_base`, calling it on `n` instead of on the
        // reflected direction, and replacing the Schlick weight with a constant all cleared the
        // step on their own. What costs is `sky_base` evaluated on a *computed* direction while
        // a Schlick weight is also live -- and, decisively, **where** that happens. Folding the
        // weight into one scalar first changes nothing (the compiler already schedules that).
        // Hoisting it does.
        //
        // **What the position buys is the death of `n` and `rd`.** At the end of the function
        // both have to stay live across the nine-cell gather, the probe tap and the shadow
        // march, and `sky_base`'s own demand then lands on top of that peak. Computed here they
        // die where they always died, and a single `vec3` crosses the gather instead.
        //
        // `sky_sm` is *not* folded in here: it is not known yet, and it does not need to be --
        // it is one scalar multiplied at the return. So the split is exactly the direction half
        // early and the gate half late.
        var spec_pre = vec3<f32>(0.0);
        if SPEC_SKY_SPECULAR && do_spec {
            spec_pre = sky_base(reflect(rd, n), 1.0)
                * schlick_ground(clamp(dot(n, -rd), 0.0, 1.0));
        }
        let axis = normal_id >> 1u;
        let box_min = c.origin + vec3<f32>(v) * vs;
        let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));

        // Eight per block since batch 14, not six: normal states 6 and 7 are the two
        // cross-quad planes and have to be addressable at the same stride. Entries 0..5 are
        // untouched, so every cube renders exactly as it did. Packed since batch 36, which is
        // why the layer comes out through `face_layer` and never off the word.
        let face_word = block_faces[id * 8u + normal_id];
        let layer = face_layer(face_word);
        // Batch 36. Without it every block samples the identical 0..1 of its layer, in phase,
        // forever -- and on a grassy slope `GRASS_SIDE`'s dirt band lines up block after block
        // and reads as chevrons. The class is a property of the layer (`block::PERM_CLASS`),
        // so a texture that cannot survive being turned simply never is.
        var uv = face_uv(normal_id, local);
        if SPEC_TEX_VARIATION && (frame.flags & FLAG_TEX_VARIATION) != 0u {
            uv = permute_uv(uv, face_perm(face_word), perm_key(box_min, normal_id));
        }
        var fa: f32;
        var fb: f32;
        var ta: vec3<i32>;
        var tb: vec3<i32>;
        if axis == 0u {
            fa = local.y; fb = local.z;
            ta = vec3<i32>(0, 1, 0); tb = vec3<i32>(0, 0, 1);
        } else if axis == 1u {
            fa = local.x; fb = local.z;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 0, 1);
        } else {
            fa = local.x; fb = local.y;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 1, 0);
        }

        // One pixel's footprint in texels, used to pick a mip and kill shimmer at range.
        // A grazing view stretches that footprint along the surface by 1/|dot(n, rd)|, which
        // is where the mid-ground moire comes from. Clamp the stretch to 4x (two mip levels)
        // so the ground stops shimmering without going to mush.
        let grazing = max(abs(dot(n, rd)), 0.25);
        let footprint = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs / grazing;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
        // The atlas's **alpha is a tint mask**, not opacity: 1 on the texels a biome may
        // recolour -- grass's top, the green fringe of its side, both leaves -- and 0
        // everywhere else, the dirt half of that same side texture included. It costs
        // nothing at all: the channel was already there holding 255 that nothing read, and
        // `textures::downsample` already carried it down the mip chain averaged linearly
        // and separately from the colour.
        let texel = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod);
        var albedo = texel.rgb;
        // Before the light term, so a tinted leaf is a different *material* and not a
        // differently lit one -- the sun, the sky and the bounce all still act on it, and
        // the reflected and refracted rays get the same albedo the primary one did because
        // they come through this same function.
        // **For a foliage block this alpha is opacity, not the batch-12 tint mask**, and a
        // blade is tinted over its whole texel -- the mask is implied 1. Reading it as a
        // mask instead would tint a tuft by how *opaque* it is, fading the biome's colour
        // out exactly at the soft edges a mip produces.
        let tint_mask = select(texel.a, 1.0, foliage);
        if SPEC_TINT && (frame.flags & FLAG_TINT) != 0u && tint_mask > 0.0 {
            // Mask and strength both scale how far the multiplier is taken, so a filtered
            // edge between fringe and dirt -- or a distant mip where the two have blurred
            // together -- fades the tint out with it instead of stepping.
            let k = tint_mask * frame.tint_strength;
            albedo = albedo * mix(vec3<f32>(1.0), biome_tint(hit.xz), k);
        }
        // Batch 75, roadmap A3's land half: wet sand. An albedo term and nothing else --
        // wetness *is* a darker albedo, and authoring it here rather than at the light
        // terms is what lets the reflected, refracted and pane-through calls of this same
        // function all read the same beach (they pass through with the same `hit`). The
        // whole term's gamma runs *through* what it is multiplied against, which is the
        // meadow lesson routed correctly for once: there is no compensation here, only a
        // material that now exists.
        if SPEC_SHORE_WET && id == SAND_ID {
            let wet = 1.0 - smoothstep(frame.sea_level, frame.sea_level + WET_BAND, hit.y);
            albedo = albedo * mix(1.0, WET_DARK, wet);
        }
        // 97d (`--foliage-rich`): the references' ground cover reads as *several
        // expressions* where ours reads as one -- here a stand of straw-dry blades,
        // there a patch a touch darker, and once in a great while a wildflower. Hashed
        // per voxel (one tint for a blade's whole quad, no texel-scale moire at any
        // mip), and an **albedo** term exactly like the biome tint above it: a material
        // varies, the light engine keeps doing the lighting. Grass and meadow tops on
        // `+Y` only -- jitter through a side face would stripe the fringe -- and
        // cross-quads everywhere they stand, for they *are* the ground cover.
        if SPEC_FOLIAGE_RICH
            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))
        {
            let hr = hash_alpha(box_min);
            let hv = hash_alpha(box_min + vec3<f32>(17.31, 7.77, 3.03));
            var rich = vec3<f32>(1.0);
            if hr < 0.16 {
                // straw-dry stand -- the references' seeded, tanned tops.
                rich = vec3<f32>(1.30, 1.10, 0.55);
            } else if hr > 0.97 {
                // a rare wildflower pink -- rare enough that a field reads, not a polka.
                rich = vec3<f32>(1.35, 0.72, 0.95);
            }
            albedo = albedo * rich * mix(0.88, 1.10, hv);
        }
        // 97e (`--canopy-relief`): the references' canopies are *grain*, not planes of
        // one green -- bright sun-kissed clumps floating over a dark interior. The
        // clump field is the deck's own value noise, world-anchored with a Y shear so
        // neighbouring layers of the same canopy do not line up; squaring isolates the
        // sunny kernels. Micro hash for the fine grain, and a sun-facing modulation so
        // the relief reads lit rather than painted. Same albedo law as its sibling
        // above: the leaf is a different green here, and the light engine still lights.
        if SPEC_CANOPY_RELIEF && is_cutout_id(id) {
            let clump = cloud_noise(box_min.xz * 0.21
                + vec2<f32>(box_min.y * 0.313, box_min.y * 0.173));
            let relief = mix(0.72, 1.34, clump * clump);
            let micro = mix(0.92, 1.08, hash_alpha(box_min));
            let toward_sun = clamp(dot(n, frame.sun_dir) * 0.5 + 0.5, 0.0, 1.0);
            albedo = albedo * relief * micro * mix(0.84, 1.0, toward_sun);
        }

        // Light, two ways. A cube face has a 3x3 of air in front of it and gets the
        // per-corner gather batch 3 built; a cross-quad has no face, no tangent pair and no
        // neighbourhood -- `gather_face` derives its two tangents from `axis`, which a
        // diagonal does not have.
        //
        // So a tuft takes the flood's own answer at the voxel it stands in, flat, with no
        // AO. It can do that precisely because it does not occlude: the light flood runs
        // through a foliage voxel, so the value stored there is the light arriving at the
        // blade rather than a zero left behind by a solid cell. Minecraft shades its
        // cross-quads flat too, and extending the corner gather to a diagonal is a batch of
        // its own rather than a line of this one.
        // Batch 63, roadmap R7. **Once per `shade_hit` and not once per corner**, because it
        // depends on the face and not on where in the face the pixel landed -- the same
        // property the nine-cell gather's own comment leans on. Under `SPEC_PROBE_AMBIENT` the
        // whole ambient leaves the module and this folds out with it; under a cleared
        // `SPEC_SKY_TINT` it folds to `SKY_TINT` and every site below is the pre-63 line.
        //
        // **After the cross-quad flip and not before it.** A blade is lit from whichever side
        // the eye is on, so `n` is not final until `shade_hit` has turned it toward the ray --
        // and a tuft's normal is horizontal, which is the one case where taking the sky from
        // the wrong side would take it from the wrong half of the gradient.
        let sky_tint = surface_sky_fill(n);
        var ao: f32 = 1.0;
        var sky_sm: f32 = 0.0;
        var amb: vec3<f32> = vec3<f32>(0.0);
        if foliage {
            let packed = light_at(ci, vec3<i32>(v));
            let sk = light_curve(light_sky(packed)) * frame.daylight;
            sky_sm = sk;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed)), sk, sky_tint);
                } else {
                    let bk = light_curve(light_block_level(packed));
                    amb = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                }
            }
        } else if !smooth_light {
            // ---- flat lighting, for a hit no primary ray ever sees (batch 45) ----
            //
            // **The same model the cross-quad above takes, and for a weaker reason that is
            // still good enough.** A tuft is shaded flat because it has no face to gather
            // across; this is shaded flat because nobody can tell. What arrives here is a
            // surface seen *in* a reflection, *through* refracting water, or *behind* a pane
            // -- in the first case Fresnel-weighted at a grazing angle off a wave-perturbed
            // normal, in the second multiplied by `exp(-extinction * depth)` on the way back,
            // in the third dimmed by the pane's own texel. Per-corner smoothing and AO are a
            // cue about *where a face meets its neighbours*, and all three of those transports
            // destroy exactly that cue before the pixel is written.
            //
            // **What it removes is the nine lookups, not the arithmetic.** `gather_face`'s
            // comment calls the grid walk the expensive half, and batch 44 is why this is the
            // line being cut: at `terraces` the traced reflection is 4.178 ms, cutting its
            // reach to 512 is *bit-exact* and saves 0.256, and cutting its shadow budget to
            // zero saves nothing -- so the money is the shading at the end of a short ray, and
            // in `shade_hit` the shading that costs memory is this.
            //
            // The air cell in front of the face, which is where light lives: a solid voxel's
            // own cell holds none, so this is `light_at(ci, v + n)` and never `light_at(ci, v)`.
            // Local when the face is not on the chunk's own boundary, which is the same test
            // and the same reason the gather below has one.
            let air_v1 = vec3<i32>(v) + vec3<i32>(n);
            var packed1: u32;
            if all(air_v1 >= vec3<i32>(0)) && all(air_v1 <= vec3<i32>(63)) {
                packed1 = light_at(ci, air_v1);
            } else {
                packed1 = world_sample(box_min + vec3<f32>(0.5 * vs) + n * vs).x;
            }
            let sk1 = light_curve(light_sky(packed1)) * frame.daylight;
            sky_sm = sk1;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed1)), sk1, sky_tint);
                } else {
                    let bk1 = light_curve(light_block_level(packed1));
                    amb = mix(sky_tint, BLOCK_TINT, bk1 / max(sk1 + bk1, 1e-4)) * max(sk1, bk1);
                }
            }
            // `ao` stays 1.0. AO is a contact shading cue and there is no contact to read at
            // one cell; leaving it at 1 is the cross-quad's own answer rather than a new one.
        } else {
            // ---- smooth lighting ----
            // The 3x3 of air cells in front of the face. Each of the four face corners averages
            // the 2x2 that touches it, and all four of those are subsets of this one
            // neighbourhood, so nine lookups feed the light and the AO together.
            //
            // **This block is 7% to 72% of `resolve` depending on the vantage, and batch 47
            // priced it and failed to make it cheaper.** Replacing it with one lookup -- the
            // flat model batch 45 gave secondary hits -- is **-1.764 ms at `cave` (72% of the
            // pass), -0.916 at `default`, -0.904 at `edits`, -0.899 at `canopy`, -0.687 at
            // `lod`, -0.599 at `terraces` and -0.065 at `coastline`**, where a frame of water
            // and sky barely reaches it. That is the ceiling on anything aimed here.
            //
            // **The nine samples depend on `(ci, v, normal_id)` and not on where in the face
            // the pixel landed** -- only `fa` and `fb` below are per-pixel -- so every pixel
            // covering one block face recomputes them identically, and at the bottom of the
            // `default` frame a block is about twenty pixels across. That looked like a
            // four-hundred-fold redundancy worth caching per face in workgroup memory.
            //
            // **It is not, and one diagnostic said so before anything was built.** Pointing all
            // nine reads at the *same cell* -- identical code, 640 bytes of `resolve`, perfect
            // sharing as far as the memory system is concerned -- recovers only **-0.191 ms at
            // `cave`, -0.123 at `default`, -0.106 at `canopy`, -0.092 at `edits`: 10% to 13% of
            // the ceiling above.** So this block is **not memory-bound**; ~88% of its cost is
            // the instruction stream -- eighteen `light_curve` evaluations, four corner
            // reciprocals, the AO rule and three bilinear mixes, none of them a hotspot on its
            // own. A cache would have paid barriers and a tag compare to chase a tenth of a
            // millisecond. See `docs/gpu.md`.
            let air_c = box_min + vec3<f32>(0.5 * vs) + n * vs;
            let step_a = vec3<f32>(ta) * vs;
            let step_b = vec3<f32>(tb) * vs;
            // The neighbourhood is nine voxels of *this* chunk, so it only needs the grid walk
            // when it reaches past the chunk's own 64^3. That is a thin border of faces, and
            // the test is uniform across the nine, so the branch does not diverge within them.
            let air_v = vec3<i32>(v) + vec3<i32>(n);
            let span = abs(ta) + abs(tb);
            let local_only = all(air_v - span >= vec3<i32>(0)) && all(air_v + span <= vec3<i32>(63));
            var nb: array<vec2<u32>, 9>;
            if local_only {
                gather_face(ci, air_v, ta, tb, &nb);
            } else {
                for (var i = 0u; i < 9u; i = i + 1u) {
                    nb[i] = world_sample(air_c + step_a * (f32(i % 3u) - 1.0) + step_b * (f32(i / 3u) - 1.0));
                }
            }
            var sky_n: array<f32, 9>;
            // Batch 59. Two arrays, one per arm of `SPEC_LIGHT_RGB`, and only one of them
            // survives the pipeline fold. The scalar one is the pre-59 code untouched, which is
            // what makes `--no-light-rgb` bit-exact by construction rather than by a claim about
            // how naga sums a vec3 whose three lanes happen to be equal.
            var blk_n: array<f32, 9>;
            var open_n: array<f32, 9>;
            // **The OR of the nine cells' block nibbles, and it is what keeps the shipping arm
            // affordable.** Three channels means twenty-seven `light_curve` evaluations where
            // the pre-59 build had nine, and batch 59's first build paid that at every shaded
            // pixel in the world: **+0.835 ms of a 2.655 ms `resolve` at `cave`**, a vantage
            // holding no emitter at all, where every one of those twenty-seven returns zero.
            //
            // **Skipping them is exact and not an approximation**, which is batch 51's own
            // argument about this block arrived at from the other side. With all nine block
            // nibbles zero every corner's `bkv` is exactly zero, so `bk` is zero, `mix` at
            // weight zero returns `SKY_TINT` to the bit and `max(sk, 0.0)` is `sk` -- the whole
            // block term reduces to `SKY_TINT * sk`, which is what the `else` arm writes. No
            // constant is authored and no pixel moves.
            //
            // **And it has no seam, which the chunk-level version of this would have.** The
            // flood is chunk-local, but this neighbourhood is not: at a chunk boundary three of
            // the nine cells come from the chunk next door through `world_sample`, so a bit
            // saying *this* chunk has no emitter would drop a lamp standing just across the
            // seam. The test is on the cells actually gathered, so there is nothing to get
            // wrong. Batch 51's rejected chunk-flag gate is the other half of that lesson: it
            // was exact and never fired, because a chunk 64 blocks tall holds the lit surface
            // above the cave.
            //
            // The branch is uniform across a workgroup wherever it matters -- a sealed room is
            // all lit and open country is all unlit -- and diverges only at the edge of a
            // lamp's reach, where both arms are paid for one tile's worth of pixels.
            var any_blk = 0u;
            for (var i = 0u; i < 9u; i = i + 1u) {
                sky_n[i] = light_curve(light_sky(nb[i].x)) * frame.daylight;
                // Batch 56. Block light is part of what a pre-composed field carries, so these
                // nine `light_curve` evaluations are nine of the eighteen batch 47 named as the
                // instruction cost of this block. **Batch 59 made them twenty-seven on the
                // shipping arm**, which is the one term this batch adds to the saturated
                // resource and the reason it is benched at `cave` rather than assumed free.
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        // Batch 73 (roadmap P11): folded out of the build whose world holds
                        // no emitter at all, where the running `or` is provably 0 -- the
                        // `if` shape, for batch 38's reason: the constant reaches this use.
                        if SPEC_EMITTER_GATHER {
                            any_blk = any_blk | (nb[i].x & 0xFFFu);
                        }
                    } else {
                        blk_n[i] = light_curve(light_block_level(nb[i].x));
                    }
                }
                open_n[i] = 1.0 - f32(nb[i].y);
            }
            // The centre cell's three curves, which every corner shares. **The other twelve
            // are taken inside the corner loop and not cached in a ninth-element array, which
            // is the opposite of what the sky and scalar paths beside it do** -- and the
            // reason is `shaderstats` rather than taste. An `array<vec3<f32>, 9>` here put
            // `resolve`'s shared memory at **16,896 bytes against 15,872**, and shared memory
            // is what decides how many workgroups an SM can hold at once. The cached version
            // evaluates nine curves and the uncached one thirteen, and the uncached one is
            // faster, because on this pass occupancy is worth more than four `light_curve`
            // calls. Batch 47 found the same shape from the other end: pointing all nine
            // gather reads at one cell recovered a tenth of what removing them did, so this
            // block is instruction-bound and not memory-bound, and trading arithmetic for
            // residency is the trade that pays.
            var blk4 = vec3<f32>(0.0);
            if SPEC_EMITTER_GATHER && SPEC_LIGHT_RGB && !SPEC_PROBE_AMBIENT && any_blk != 0u {
                blk4 = light_curve3(light_block_rgb(nb[4].x));
            }

            // Per corner: the mean of the light its 2x2 can see, tinted by whichever source
            // dominates there. The mean is taken after `light_curve`, not before, so the
            // interpolation happens in radiance like every other blend since batch 2.
            let ao_on = (frame.flags & FLAG_AO) != 0u;
            var amb_c: array<vec3<f32>, 4>;
            var sky_c: array<f32, 4>;
            var ao_c: array<f32, 4>;
            for (var k = 0u; k < 4u; k = k + 1u) {
                let ia = u32(4 + (i32(k & 1u) * 2 - 1));
                let ib = u32(4 + (i32(k >> 1u) * 2 - 1) * 3);
                let ic = ia + ib - 4u;
                // Solid cells hold no light and stay out of the mean. Cell 4 is the air the ray
                // arrived through, so it always counts and the divisor is never zero.
                let inv = 1.0 / (1.0 + open_n[ia] + open_n[ib] + open_n[ic]);
                let sk = (sky_n[4] + sky_n[ia] * open_n[ia] + sky_n[ib] * open_n[ib] + sky_n[ic] * open_n[ic]) * inv;
                sky_c[k] = sk;
                // One scalar level used to carry both sources and then wear the sky's tint,
                // which was invisible while lit caves were crushed to black and is not
                // invisible now: glowstone lit them blue. Blending per corner rather than per
                // face also stops a torch on one side of a face warming the far side of it.
                if !SPEC_PROBE_AMBIENT {
                    // The **radiance** of each channel is what the corner averages, and then
                    // the tint is derived from the average. Averaging a chroma instead would
                    // give a corner between a red lamp and a blue one the mean of two hues at
                    // the brightness of one, where what it actually receives is the sum.
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER && any_blk != 0u {
                            let ba = light_curve3(light_block_rgb(nb[ia].x));
                            let bb = light_curve3(light_block_rgb(nb[ib].x));
                            let bc = light_curve3(light_block_rgb(nb[ic].x));
                            let bkv = (blk4 + ba * open_n[ia] + bb * open_n[ib] + bc * open_n[ic]) * inv;
                            amb_c[k] = block_ambient(bkv, sk, sky_tint);
                        } else {
                            // `block_ambient(vec3(0.0), sk)` written out, and equal to it bit for
                            // bit: `mix(a, b, 0.0)` is `a` and `max(sk, 0.0)` is `sk`.
                            amb_c[k] = sky_tint * sk;
                        }
                    } else {
                        let bk = (blk_n[4] + blk_n[ia] * open_n[ia] + blk_n[ib] * open_n[ib] + blk_n[ic] * open_n[ic]) * inv;
                        amb_c[k] = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                    }
                }
                // Minecraft's vertex rule: two solid sides shut the corner whatever the
                // diagonal does, since it sits behind them and cannot occlude any further.
                let s1 = 1.0 - open_n[ia];
                let s2 = 1.0 - open_n[ib];
                let occ = select(s1 + s2 + 1.0 - open_n[ic], 3.0, s1 > 0.5 && s2 > 0.5);
                ao_c[k] = select(1.0, 1.0 - occ * AO_STRENGTH, ao_on);
            }

            // Bilinear across the face. `fa` and `fb` run along `ta` and `tb`, so corner k sits
            // at fa = k & 1, fb = k >> 1.
            ao = mix(mix(ao_c[0], ao_c[1], fa), mix(ao_c[2], ao_c[3], fa), fb);
            sky_sm = mix(mix(sky_c[0], sky_c[1], fa), mix(sky_c[2], sky_c[3], fa), fb);
            if !SPEC_PROBE_AMBIENT {
                amb = mix(mix(amb_c[0], amb_c[1], fa), mix(amb_c[2], amb_c[3], fa), fb);
            }
        }

        // Batch 55. The tap is the whole batch: with the field filled at 1.0 this multiply is
        // bit-exact for every finite `amb`, so nothing in any capture moves and the reading is
        // a millisecond. Gated by the override alone -- a `frame.flags` companion would keep
        // the sample in all four of `shade_hit`'s inlined copies and the shipping build would
        // pay for a tap it never takes, which is batch 45's +1.42 ms in miniature.
        if SPEC_PROBE_AMBIENT {
            // Batch 56. The field *is* the ambient: directional, coloured and bounced, which is
            // the whole of what the three deleted terms above were approximating with two
            // scalars and a constant. Assigned rather than multiplied, which is what makes this
            // build incorrect and its picture disposable -- batch 55's multiply was the version
            // that had to stay bit-exact.
            amb = probe_tap(hit);
        } else if SPEC_PROBE_TAP {
            amb = amb * probe_tap(hit);
        }

        var visibility = 1.0;
        var direct = 0.0;
        let ndl = dot(n, frame.sun_dir);
        if ndl > 0.0 && frame.daylight > 0.01 {
            var lit = 1.0;
            if (frame.flags & FLAG_SHADOWS) != 0u {
                // Batch 102a. The disc's taps fold away whole with the bit clear, and
                // the else's line is the legacy one to the character -- the unarmed
                // pipeline holds exactly the ray it always held. `lit` becomes a
                // fraction in the arm, which is the whole point: `direct` and the
                // envelope below both already multiply by it.
                lit = select(1.0, 0.0, shadow_ray(hit + n * (0.02 * vs), frame.sun_dir, shadow_dist));
                // Batch 37. The march stops at `shadow_dist` and nothing shadowed anything past
                // it, so a mountain standing in the shadow of a bigger one behind it was drawn
                // fully lit and the whole far field read as flat, evenly lit terrain at exactly
                // the low sun that makes the near field worth looking at.
                //
                // **The envelope takes over where the march stops, and `terrain_shade_beyond`
                // is why that is a partition rather than a blend** -- passing the same
                // `shadow_dist` the ray just used removes every occluder the march already
                // answered from the maximum, so the two cover [0, dist] and [dist, reach) with
                // no gap and no double count. The proof is at that function; the reason it is
                // not `terrain_shade(hit)` is there too.
                //
                // Under `lit > 0.0` for the reason `cloud_shade` is below: a face the march
                // already put in shadow cannot be shadowed further, so the tap is paid only by
                // pixels that are still lit. Gated by `FLAG_SHADOWS` because this *is* a
                // shadow -- `--ambient` asks for none and must not get one from the envelope.
                if SPEC_DISTANT_SHADOWS && (frame.flags & FLAG_DISTANT_SHADOWS) != 0u && lit > 0.0 {
                    lit = lit * terrain_shade_beyond(hit, shadow_dist);
                }
            }
            // The deck's own shadow (batch 11), through the same function the shaft integral
            // uses -- if a cloud occludes a beam it has to occlude the ground under the beam,
            // and one function is what keeps the two from disagreeing. A face the world
            // shadow ray already caught pays nothing, and `cloud_shade` returns exactly 1.0
            // by an early exit under `--cloud-shadow 0`, where `lit * 1.0` is `lit` to the
            // bit. The footprint is this pixel's own at this surface, in blocks;
            // `cloud_shade` takes the wider of it and the sun's own penumbra and converts.
            // It is the same quantity the deck's band limit takes for a sky ray, measured
            // here instead of where a view ray meets the plane.
            if lit > 0.0 {
                let cloud_fp = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y)
                    * exp2(frame.mip_bias);
                lit = lit * cloud_shade(hit, cloud_fp);
            }
            // Gated by the smoothed sky term for the same reason it was gated by the flat
            // one — sun must not leak onto a face the shadow ray missed — but the gate no
            // longer puts a per-voxel step back into an otherwise smooth face.
            visibility = lit;
            direct = lit * ndl * sky_sm;
            // Batch 90, roadmap A9 (`--caustics`). **The sheets are the wave field's own**:
            // project the hit up the sun ray to the sea plane and read the field's first two
            // deep-water octaves there, so what lands on the bottom is exactly what the
            // waterline shades a few blocks up, refocused by a depth's worth of Snell. Two
            // octaves, not the full schedule -- the caustic is a woven interference pattern
            // by physics, and the fourth octave contributes λ/λ₀ < 1/10 to a read that is
            // already sharpening two magnitudes of slope product. Gated on the waterline's
            // own facts (`sky_sm`: under a roof there is no sun to focus) and folded out
            // of the shipping build, which makes `--no-caustics` bit-exact by construction
            // rather than by retune -- every one of the constants below was authored for
            // the by-eye pass, and none of them moves a row of the control.
            if SPEC_CAUSTICS {
                let cdepth = frame.sea_level - hit.y;
                if cdepth > 0.0 && cdepth < CAUSTIC_DEPTH && direct > 0.0 {
                    // Up the sun ray from the floor to the waterline: `cdepth / sun.y` far,
                    // `sun.xz` of that sideways. A flat-sea run (`--wave-amp 0`) gives zero
                    // slope, the product below is zero and the multiply is the identity --
                    // the anti-dephase control (`--anim-rate`) therefore also gates this.
                    let sp = hit.xz - frame.sun_dir.xz * (cdepth / max(frame.sun_dir.y, 1e-3));
                    let a = frame.wave_amp;
                    let l0 = frame.wave_scale;
                    let ph = frame.time * frame.wave_speed * WAVE_TAU;
                    // Isotropic on purpose: the caustic filter width is tied to the water
                    // column, not to a view ray that may not even exist past the refracted
                    // march -- the marching pixel's footprint was already paid for above.
                    let g0 = wave_octave(sp, WAVE_D0, l0, a * 0.42, ph, CAUSTIC_PX,
                                         vec2<f32>(0.0), 0.0, false);
                    let g1 = wave_octave(sp, WAVE_D1, l0 / 2.17, a * 0.27, ph * 1.4731,
                                         CAUSTIC_PX, vec2<f32>(0.0), 0.0, false);
                    // Sheets where two wavetrains' slopes cross: the product of their
                    // magnitudes, squared once so the middles fall and the crossings stand.
                    // `v` is the slope vector summed isotropically -- the physics object is
                    // |∇h| per train, and neither train has a view direction to aniso to.
                    let r0 = g0.x * g0.x + g0.y * g0.y;
                    let r1 = g1.x * g1.x + g1.y * g1.y;
                    let sheets = r0 * r1 * CAUSTIC_GAIN;
                    direct = direct * (1.0 + sheets * exp(-cdepth * CAUSTIC_FADE));
                }
            }
        }

        // Cool sky ambient against warm direct sun, so shadowed faces read as shadow
        // rather than just slightly darker. The sun tint was authored as a display colour
        // and is decoded here; in linear it stays near-neutral under full sun.
        // Batch 56. `face_shade` is a hardcoded 1.00/0.80/0.62/0.50 per normal standing in for
        // directional occlusion, and a directional field is the thing that would actually supply
        // it -- so it is one of the terms the architecture removes rather than pays for.
        // Batch 57. `face_shade` is a hardcoded 1.00/0.80/0.62/0.50 per normal standing in
        // for the whole of directional occlusion, and the baked cube is what actually supplies
        // it: one trilinear tap into a field that knows how much sky this position sees along
        // this axis. The lattice is created holding that same table, so the arm below is a
        // pure revert and an unbaked texel renders the pre-57 frame.
        // One tap for both baked fields. Gated on the two overrides together so that a build
        // with neither takes no sample at all -- which is what makes the pair of controls the
        // pre-batch-57 frame rather than the pre-57 frame plus a fetch nobody reads.
        var probe = vec4<f32>(1.0);
        if !SPEC_PROBE_AMBIENT && (SPEC_PROBE_CUBE || SPEC_PROBE_BOUNCE) {
            probe = probe_field(hit + n * PROBE_NORMAL_BIAS, normal_id);
        }
        var shade = 1.0;
        if !SPEC_PROBE_AMBIENT {
            shade = select(face_shade(normal_id), 1.0, SPEC_LIGHTING_REPAIR);
            if SPEC_PROBE_CUBE {
                shade = shade * probe.a;
            }
        }
        let ambient_rgb = amb * 0.50;
        let sun_rgb = vec3<f32>(1.0000, 0.8469, 0.5647) * (direct * select(0.52, 0.78, SPEC_LIGHTING_REPAIR));
        // The floor is kept out of the tinted term and is only faintly cool: it is the sole
        // light in an unlit cave, and folded into the sky tint it turned stone navy. It is
        // also kept out of the AO product, which the nearest-corner version could get away
        // with including: a floor of 0.08 times a fully occluded 0.25 lands back under the
        // display's first code values, and bilerp puts that over a quarter of every face
        // instead of a rim. Nothing about it is occludable anyway — it is what stands in
        // for the bounce light the two-source model never simulates.
        // Batch 56. The floor's own comment says it "is what stands in for the bounce light the
        // two-source model never simulates", so a field carrying a real bounce is exactly what
        // deletes it. The second of the two constants this architecture is meant to retire.
        // Batch 58. The floor's own comment is the acceptance test for roadmap R2 and this is
        // the line that answers it: the constant stops being the bounce and becomes the
        // *calibration* of one, exactly as `face_shade` did one rung earlier. `probe.rgb` is a
        // per-channel multiplier the bake normalised so that grass returns the floor's own
        // *luminance* -- so over grass this expression changes hue and not brightness, and
        // snow, sand, stone and open water depart in both. The identity that is per channel is
        // a different one: an unbaked texel returns 1.0 and renders the pre-58 frame to the
        // bit. `probe::BOUNCE_REF`'s note separates the two. The same `PROBE_NORMAL_BIAS`
        // address the occlusion used, because it is the same tap.
        var floor_rgb = vec3<f32>(0.0);
        if !SPEC_PROBE_AMBIENT {
            floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;
            if SPEC_PROBE_BOUNCE {
                floor_rgb = floor_rgb * probe.rgb;
            }
            // Batch 60, roadmap R3, and it is ADR 0001's retry condition rather than a new
            // idea. That ADR rejected the elevation form factor on a ceiling rather than on an
            // implementation: `frame.ambient` is 0.08, so the line above is about 7% of the
            // light on a lit surface and **no redistribution of it can be worth more than
            // that**. Batch 60's own transport half measured 0.372 MAE at `default` against the
            // 2.87 that ADR already rejected by eye, which is the same ceiling found twice. The
            // way out the ADR names is more energy, and this is the line that carries it.
            //
            // **Added rather than folded into the constant**, so the authored floor stays
            // exactly what it was and goes on being the sole light in an unlit cave. Every
            // factor here is zero where a cave is: `frame.daylight` at night, `probe.a` -- the
            // baked sky occlusion -- underground, and `probe.rgb` has already been driven to
            // 1.0 by `believe` wherever no sky reaches. So the term appears where sunlight
            // lands on ground and nowhere else, which is what an indirect bounce *is*, and the
            // pre-60 frame comes back to the bit by clearing one flag.
            //
            // **The gate is `sky_sm` and the first draft of this batch got it wrong**, in a way
            // worth keeping because the mistake is the field's central design decision read
            // backwards. The draft gated on `probe.a * frame.daylight`, reasoning that a cave
            // sees no sky. But the field stores **occlusion normalised to 1 in the open**, so
            // that every absence of it is exact -- an unbaked texel, a coarse LOD and a probe
            // the bake has no opinion about all return **1.0**, which is *unoccluded*, not
            // *dark*. The bake reads `WorldGen::height` and never the voxels, so it has no
            // opinion about anything underground at all. The result measured 921,600 pixels at
            // `cave` and at `lamps`: full sunlight bouncing around a sealed chamber.
            //
            // `sky_sm` is the right quantity and was already in scope two lines up, gating
            // `direct` for the same reason. It is the flood's own per-voxel answer to how much
            // sky reaches this surface, it is zero in a cave and in a sealed room, and it
            // already carries `frame.daylight` -- so this term needs neither of the two factors
            // the draft multiplied in, and at night it goes to zero through the same path the
            // sun does. **The authored floor above is untouched and goes on being the sole
            // light in an unlit cave**, which is what keeps the two lines a pair rather than a
            // replacement.
            if SPEC_PROBE_SUN {
                floor_rgb = floor_rgb
                    + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
            }
        }
        // ---- batch 65, roadmap R6: the sky's own reflection, on everything ----
        //
        // **Every opaque surface in this world has been perfectly diffuse since batch 1.** Water
        // and glass have traced real reflections since batches 8 and 38b; stone, snow, soil and
        // bark have had none at all, which is the largest single gap between this renderer and a
        // path-traced one now that the ambient rungs have landed.
        //
        // **This term is added outside `albedo`, and that is the whole reason R6 was worth
        // opening where R3 and R7 were capped.** A specular reflection off a dielectric is not
        // tinted by the diffuse albedo and is not a redistribution of the ambient -- it is new
        // energy on top of the line above. R3's rung modulated the floor, which `Config::ambient`
        // pins at about 7% of a lit surface; R7's carried `SKY_TINT`'s luminance by construction
        // so only hue could move. Neither could have been worth much before it was built. This
        // one is bounded by Fresnel instead: `GROUND_F0`'s 4% at normal incidence, but Schlick
        // reaches **20% at cos 0.3 and 61% at cos 0.1**, and a ground plane seen from eye height
        // is at grazing incidence over most of its extent. That is the ceiling question asked
        // before building rather than after, which is what R6's entry demanded.
        //
        // **The gate is `sky_sm`, and batch 60 is why it is not `probe.a`.** That batch's first
        // draft gated its bounce on the baked occlusion and lit a sealed chamber with 921,600
        // pixels, because the field stores occlusion *normalised to 1 in the open* -- an unbaked
        // texel, a coarse LOD and anything underground all return 1.0, which is unoccluded and
        // not dark. `sky_sm` is the flood's own per-voxel answer to how much sky reaches this
        // surface, it is exactly zero in a cave and in a sealed room, and it already carries
        // `frame.daylight`, so this term goes to zero at night through the same path the sun
        // does. The identical reasoning, one rung later, and it is the third time this gate has
        // been the right one.
        //
        // **`sky_base` and not `sky_color`.** The invariant in `CLAUDE.md` is that `sky_color`
        // takes a ray origin and `sky_base` must not; what is wanted here is the sky's radiance
        // in a direction, with no deck intersected and no origin to intersect it from. It also
        // already handles a mirror direction that points below the horizon -- its last line
        // mixes toward `horizon * 0.26` as `rd.y` goes negative -- so a ceiling, an overhang and
        // an underside need no special case and reflect dark ground rather than sky.
        //
        // **Two things this rung does not do, both deliberate and both recorded rather than
        // hidden.** There is no roughness: the lookup is a mirror, and what makes that tolerable
        // is that the sky is low-frequency, which is exactly the approximation R6's entry says a
        // low-order probe field would also be making. And `sky_base`'s sun disc is *unshadowed*,
        // so a slope the shadow ray rejected can still catch a glint -- the disc is narrow
        // (`smoothstep(0.9975, 0.9990)`) so it is small, but it is wrong, and separating it out
        // would need a fourth sky function rather than a condition.
        if SURFACE_DEBUG == 1u { return n * 0.5 + vec3<f32>(0.5); }
        if SURFACE_DEBUG == 2u { return vec3<f32>(visibility); }
        if SURFACE_DEBUG == 3u { return sun_rgb; }
        if SURFACE_DEBUG == 4u { return ambient_rgb * ao + floor_rgb; }
        let spec_rgb = spec_pre * (sky_sm * SKY_SPEC_GAIN);
        return surface_radiance(albedo, surface_light(ambient_rgb, sun_rgb, ao, floor_rgb, shade), spec_rgb, id);
}

// EXPERIMENT: gather/shadows/probes finish before UV, atlas and material work.
// LOD stays early so the view direction need not survive just to calculate it.
// The batch-67 specular hoist and final multiplication order are preserved.
// Compiler scheduling can undo this or increase pressure: no register saving is claimed.
fn shade_hit_compact(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool) -> vec3<f32> {
        let c = chunks[ci];
        let vs = c.voxel_size;
        let foliage = normal_id >= NORMAL_CROSS_A;
        var n = normal_of(normal_id);
        if foliage && dot(n, rd) > 0.0 {
            n = -n;
        }
        var spec_pre = vec3<f32>(0.0);
        if SPEC_SKY_SPECULAR && do_spec {
            spec_pre = sky_base(reflect(rd, n), 1.0)
                * schlick_ground(clamp(dot(n, -rd), 0.0, 1.0));
        }
        let grazing = max(abs(dot(n, rd)), 0.25);
        let footprint = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs / grazing;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
        var light_rgb: vec3<f32>;
        var face_scale: f32;
        var spec_value: vec3<f32>;
        var material_ndl: f32;
        {
        let axis = normal_id >> 1u;
        let box_min = c.origin + vec3<f32>(v) * vs;
        let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));
        var fa: f32;
        var fb: f32;
        var ta: vec3<i32>;
        var tb: vec3<i32>;
        if axis == 0u {
            fa = local.y; fb = local.z;
            ta = vec3<i32>(0, 1, 0); tb = vec3<i32>(0, 0, 1);
        } else if axis == 1u {
            fa = local.x; fb = local.z;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 0, 1);
        } else {
            fa = local.x; fb = local.y;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 1, 0);
        }
        let sky_tint = surface_sky_fill(n);
        var ao: f32 = 1.0;
        var sky_sm: f32 = 0.0;
        var amb: vec3<f32> = vec3<f32>(0.0);
        if foliage {
            let packed = light_at(ci, vec3<i32>(v));
            let sk = light_curve(light_sky(packed)) * frame.daylight;
            sky_sm = sk;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed)), sk, sky_tint);
                } else {
                    let bk = light_curve(light_block_level(packed));
                    amb = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                }
            }
        } else if !smooth_light {
            let air_v1 = vec3<i32>(v) + vec3<i32>(n);
            var packed1: u32;
            if all(air_v1 >= vec3<i32>(0)) && all(air_v1 <= vec3<i32>(63)) {
                packed1 = light_at(ci, air_v1);
            } else {
                packed1 = world_sample(box_min + vec3<f32>(0.5 * vs) + n * vs).x;
            }
            let sk1 = light_curve(light_sky(packed1)) * frame.daylight;
            sky_sm = sk1;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed1)), sk1, sky_tint);
                } else {
                    let bk1 = light_curve(light_block_level(packed1));
                    amb = mix(sky_tint, BLOCK_TINT, bk1 / max(sk1 + bk1, 1e-4)) * max(sk1, bk1);
                }
            }
        } else {
            let air_c = box_min + vec3<f32>(0.5 * vs) + n * vs;
            let step_a = vec3<f32>(ta) * vs;
            let step_b = vec3<f32>(tb) * vs;
            let air_v = vec3<i32>(v) + vec3<i32>(n);
            let span = abs(ta) + abs(tb);
            let local_only = all(air_v - span >= vec3<i32>(0)) && all(air_v + span <= vec3<i32>(63));
            var nb: array<vec2<u32>, 9>;
            if local_only {
                gather_face(ci, air_v, ta, tb, &nb);
            } else {
                for (var i = 0u; i < 9u; i = i + 1u) {
                    nb[i] = world_sample(air_c + step_a * (f32(i % 3u) - 1.0) + step_b * (f32(i / 3u) - 1.0));
                }
            }
            var sky_n: array<f32, 9>;
            var blk_n: array<f32, 9>;
            var open_n: array<f32, 9>;
            var any_blk = 0u;
            for (var i = 0u; i < 9u; i = i + 1u) {
                sky_n[i] = light_curve(light_sky(nb[i].x)) * frame.daylight;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER {
                            any_blk = any_blk | (nb[i].x & 0xFFFu);
                        }
                    } else {
                        blk_n[i] = light_curve(light_block_level(nb[i].x));
                    }
                }
                open_n[i] = 1.0 - f32(nb[i].y);
            }
            var blk4 = vec3<f32>(0.0);
            if SPEC_EMITTER_GATHER && SPEC_LIGHT_RGB && !SPEC_PROBE_AMBIENT && any_blk != 0u {
                blk4 = light_curve3(light_block_rgb(nb[4].x));
            }
            let ao_on = (frame.flags & FLAG_AO) != 0u;
            var amb_c: array<vec3<f32>, 4>;
            var sky_c: array<f32, 4>;
            var ao_c: array<f32, 4>;
            for (var k = 0u; k < 4u; k = k + 1u) {
                let ia = u32(4 + (i32(k & 1u) * 2 - 1));
                let ib = u32(4 + (i32(k >> 1u) * 2 - 1) * 3);
                let ic = ia + ib - 4u;
                let inv = 1.0 / (1.0 + open_n[ia] + open_n[ib] + open_n[ic]);
                let sk = (sky_n[4] + sky_n[ia] * open_n[ia] + sky_n[ib] * open_n[ib] + sky_n[ic] * open_n[ic]) * inv;
                sky_c[k] = sk;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER && any_blk != 0u {
                            let ba = light_curve3(light_block_rgb(nb[ia].x));
                            let bb = light_curve3(light_block_rgb(nb[ib].x));
                            let bc = light_curve3(light_block_rgb(nb[ic].x));
                            let bkv = (blk4 + ba * open_n[ia] + bb * open_n[ib] + bc * open_n[ic]) * inv;
                            amb_c[k] = block_ambient(bkv, sk, sky_tint);
                        } else {
                            amb_c[k] = sky_tint * sk;
                        }
                    } else {
                        let bk = (blk_n[4] + blk_n[ia] * open_n[ia] + blk_n[ib] * open_n[ib] + blk_n[ic] * open_n[ic]) * inv;
                        amb_c[k] = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                    }
                }
                let s1 = 1.0 - open_n[ia];
                let s2 = 1.0 - open_n[ib];
                let occ = select(s1 + s2 + 1.0 - open_n[ic], 3.0, s1 > 0.5 && s2 > 0.5);
                ao_c[k] = select(1.0, 1.0 - occ * AO_STRENGTH, ao_on);
            }
            ao = mix(mix(ao_c[0], ao_c[1], fa), mix(ao_c[2], ao_c[3], fa), fb);
            sky_sm = mix(mix(sky_c[0], sky_c[1], fa), mix(sky_c[2], sky_c[3], fa), fb);
            if !SPEC_PROBE_AMBIENT {
                amb = mix(mix(amb_c[0], amb_c[1], fa), mix(amb_c[2], amb_c[3], fa), fb);
            }
        }
        if SPEC_PROBE_AMBIENT {
            amb = probe_tap(hit);
        } else if SPEC_PROBE_TAP {
            amb = amb * probe_tap(hit);
        }
        var visibility = 1.0;
        var direct = 0.0;
        let ndl = dot(n, frame.sun_dir);
        if ndl > 0.0 && frame.daylight > 0.01 {
            var lit = 1.0;
            if (frame.flags & FLAG_SHADOWS) != 0u {
                lit = select(1.0, 0.0, shadow_ray(hit + n * (0.02 * vs), frame.sun_dir, shadow_dist));
                if SPEC_DISTANT_SHADOWS && (frame.flags & FLAG_DISTANT_SHADOWS) != 0u && lit > 0.0 {
                    lit = lit * terrain_shade_beyond(hit, shadow_dist);
                }
            }
            if lit > 0.0 {
                let cloud_fp = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y)
                    * exp2(frame.mip_bias);
                lit = lit * cloud_shade(hit, cloud_fp);
            }
            visibility = lit;
            direct = lit * ndl * sky_sm;
            if SPEC_CAUSTICS {
                let cdepth = frame.sea_level - hit.y;
                if cdepth > 0.0 && cdepth < CAUSTIC_DEPTH && direct > 0.0 {
                    let sp = hit.xz - frame.sun_dir.xz * (cdepth / max(frame.sun_dir.y, 1e-3));
                    let a = frame.wave_amp;
                    let l0 = frame.wave_scale;
                    let ph = frame.time * frame.wave_speed * WAVE_TAU;
                    let g0 = wave_octave(sp, WAVE_D0, l0, a * 0.42, ph, CAUSTIC_PX,
                                         vec2<f32>(0.0), 0.0, false);
                    let g1 = wave_octave(sp, WAVE_D1, l0 / 2.17, a * 0.27, ph * 1.4731,
                                         CAUSTIC_PX, vec2<f32>(0.0), 0.0, false);
                    let r0 = g0.x * g0.x + g0.y * g0.y;
                    let r1 = g1.x * g1.x + g1.y * g1.y;
                    let sheets = r0 * r1 * CAUSTIC_GAIN;
                    direct = direct * (1.0 + sheets * exp(-cdepth * CAUSTIC_FADE));
                }
            }
        }
        var probe = vec4<f32>(1.0);
        if !SPEC_PROBE_AMBIENT && (SPEC_PROBE_CUBE || SPEC_PROBE_BOUNCE) {
            probe = probe_field(hit + n * PROBE_NORMAL_BIAS, normal_id);
        }
        var shade = 1.0;
        if !SPEC_PROBE_AMBIENT {
            shade = select(face_shade(normal_id), 1.0, SPEC_LIGHTING_REPAIR);
            if SPEC_PROBE_CUBE {
                shade = shade * probe.a;
            }
        }
        let ambient_rgb = amb * 0.50;
        let sun_rgb = vec3<f32>(1.0000, 0.8469, 0.5647) * (direct * select(0.52, 0.78, SPEC_LIGHTING_REPAIR));
        var floor_rgb = vec3<f32>(0.0);
        if !SPEC_PROBE_AMBIENT {
            floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;
            if SPEC_PROBE_BOUNCE {
                floor_rgb = floor_rgb * probe.rgb;
            }
            if SPEC_PROBE_SUN {
                floor_rgb = floor_rgb
                    + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
            }
        }
        if SURFACE_DEBUG == 1u { return n * 0.5 + vec3<f32>(0.5); }
        if SURFACE_DEBUG == 2u { return vec3<f32>(visibility); }
        if SURFACE_DEBUG == 3u { return sun_rgb; }
        if SURFACE_DEBUG == 4u { return ambient_rgb * ao + floor_rgb; }
        let spec_rgb = spec_pre * (sky_sm * SKY_SPEC_GAIN);
        light_rgb = surface_light(ambient_rgb, sun_rgb, ao, floor_rgb, shade);
        face_scale = 1.0;
        spec_value = spec_rgb;
        material_ndl = ndl;
        }
        let material_chunk = chunks[ci];
        let box_min = material_chunk.origin + vec3<f32>(v) * material_chunk.voxel_size;
        let local = clamp((hit - box_min) / material_chunk.voxel_size, vec3<f32>(0.0), vec3<f32>(1.0));
        let face_word = block_faces[id * 8u + normal_id];
        let layer = face_layer(face_word);
        var uv = face_uv(normal_id, local);
        if SPEC_TEX_VARIATION && (frame.flags & FLAG_TEX_VARIATION) != 0u {
            uv = permute_uv(uv, face_perm(face_word), perm_key(box_min, normal_id));
        }
        let texel = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod);
        var albedo = texel.rgb;
        let tint_mask = select(texel.a, 1.0, foliage);
        if SPEC_TINT && (frame.flags & FLAG_TINT) != 0u && tint_mask > 0.0 {
            let k = tint_mask * frame.tint_strength;
            albedo = albedo * mix(vec3<f32>(1.0), biome_tint(hit.xz), k);
        }
        if SPEC_SHORE_WET && id == SAND_ID {
            let wet = 1.0 - smoothstep(frame.sea_level, frame.sea_level + WET_BAND, hit.y);
            albedo = albedo * mix(1.0, WET_DARK, wet);
        }
        if SPEC_FOLIAGE_RICH
            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))
        {
            let hr = hash_alpha(box_min);
            let hv = hash_alpha(box_min + vec3<f32>(17.31, 7.77, 3.03));
            var rich = vec3<f32>(1.0);
            if hr < 0.16 {
                rich = vec3<f32>(1.30, 1.10, 0.55);
            } else if hr > 0.97 {
                rich = vec3<f32>(1.35, 0.72, 0.95);
            }
            albedo = albedo * rich * mix(0.88, 1.10, hv);
        }
        if SPEC_CANOPY_RELIEF && is_cutout_id(id) {
            let clump = cloud_noise(box_min.xz * 0.21
                + vec2<f32>(box_min.y * 0.313, box_min.y * 0.173));
            let relief = mix(0.72, 1.34, clump * clump);
            let micro = mix(0.92, 1.08, hash_alpha(box_min));
            let toward_sun = clamp(material_ndl * 0.5 + 0.5, 0.0, 1.0);
            albedo = albedo * relief * micro * mix(0.84, 1.0, toward_sun);
        }
        return surface_radiance(albedo, light_rgb * face_scale, spec_value, id);
}

fn shade_hit(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool) -> vec3<f32> {
    if SPEC_COMPACT_SHADE_HIT {
        return shade_hit_compact(ci, v, id, normal_id, hit, rd, dist, shadow_dist, smooth_light, do_spec);
    }
    return shade_hit_legacy(ci, v, id, normal_id, hit, rd, dist, shadow_dist, smooth_light, do_spec);
}

// How much bounced sunlight an open, fully lit patch of *reference* ground contributes,
// against `frame.ambient`'s 0.08 for the authored floor beside it.
//
// **Authored, and the one number in batch 60 that is.** Everything else in the term is
// derived -- the ground's albedo and the sky it sees come from the bake, the sun's strength
// from `frame.daylight`, the receiver's exposure from the same `probe.a` the face shading
// reads. What no measurement in this tree fixes is the overall strength of one bounce
// relative to the direct term, because nothing here has ever computed one.
//
// **0.10 is a doubling of the floor over lit open ground and was chosen for that**, which is
// the smallest change that clears the ceiling ADR 0001 measured: a redistribution of 0.08 was
// worth 2.87 code values by eye and was rejected, so a term that cannot at least double it
// would be the same finding a third time. It is *not* an energy-conserving derivation and
// must not be described as one -- a real one needs the sun's own radiance at the ground and
// the receiver's form factor onto it, and R3's later rungs are where that lives.
//
// Swept by eye against `--no-probe-sun`: see the ledger row.
const PROBE_SUN_GAIN: f32 = 0.10;
// The louder rung, reachable as `--probe-sun-high`. **Two constants selected by an override
// rather than one uniform**, which is the shape `SPEC_LEAF_FILL` already uses and the reason is
// `GpuFrame`: it has no pads left, appending to it costs four words and five `offset_of!`
// asserts, and a value that only ever takes two settings does not need a uniform at all. The
// override folds at pipeline build, so the arm not taken is not compiled and the sweep is free.
const PROBE_SUN_GAIN_HIGH: f32 = 0.25;

fn probe_sun_gain() -> f32 {
    var g = PROBE_SUN_GAIN;
    if SPEC_PROBE_SUN_HIGH {
        g = PROBE_SUN_GAIN_HIGH;
    }
    return g;
}

// ---- water ----

// Beyond this the two secondary rays stop paying for themselves: a water surface a few
// hundred blocks out is seen at a grazing angle, where Fresnel has already sent nearly all
// of the weight to the reflection and the refracted term has converged to the body colour.
// What fades is the *hit*, not the trace: a reflection that misses is the sky, and the sky
// answer is exact at every distance, so only a hit could show a seam.
const WATER_REFRACT_NEAR: f32 = 96.0;
const WATER_REFRACT_FAR: f32 = 192.0;
// **Swept alone in batch 44 and left where it was.** Moving the pair to 256/384 -- the fade
// half of the one build batch 40 benched -- is **-0.553 ms of `resolve` at `coastline`**,
// -0.870 at `open-sea` and **zero at `terraces`** (-0.026 +/- 0.077), because terraces' water
// is all near the eye. It is the most expensive of the three reflection levers to *look* at:
// **68,661 pixels over the sixteen vantages at max channel delta 146, and 13 of the 16 move**,
// against 31,065 at delta 86 for the reach and 273 at delta 8 for the shadow budget. What it
// takes is every reflected hit on water more than a few hundred blocks out at once, and the
// sentence above is why that shows -- the *hit* is what fades, and a hit is the only thing in
// this term that is not already exact.
const WATER_REFLECT_NEAR: f32 = 512.0;
const WATER_REFLECT_FAR: f32 = 1024.0;
// The refracted ray only has to find a bottom; the reflected one has to find a hillside,
// which is the whole reason anyone wants a reflection.
//
// **768 was swept in batch 44 against a picture and kept, and the hillside in that sentence is
// why.** The reach is real money -- at `coastline`, `resolve` falls **-0.446 ms at 512, -0.746
// at 384, -1.164 at 256 and -1.646 at 128** -- and every one of those buys the same thing: the
// headland at the right edge of that frame stops being reflected in the sea under it and shows
// sky instead. **It is gone by 512**, which moves 5,442 pixels at max channel delta 86, and no
// value below 768 keeps it. The band this truncates is the opposite of the one batch 41 found
// under the water: not nearly empty, but holding the most prominent reflection in the fixture.
//
// **And batch 40's -1.476 ms for this constant was the reach and the fade together.** That
// build moved `WATER_REFLECT_NEAR/FAR` as well; measured apart, the reach is -0.746 and the
// fade -0.553. `PERF.md` carries the corrected pair.
const WATER_REFRACT_DIST: f32 = 96.0;
const WATER_REFLECT_DIST: f32 = 768.0;
// Moved to `common.wgsl` at batch 91 -- `march.wgsl`'s two-segment bend starts rays on
// the sea plane and both files must nudge by the same epsilon; see the constant there.

// Whether a hit reached through water or glass is shaded with the nine-cell gather or with
// one lookup (batch 45). It is the *argument* the three secondary call sites pass, so the
// name reads the way the parameter does: `--no-flat-secondary` clears the flag, `ensure_spec`
// builds the pipeline with the override false, this becomes `true`, and all four call sites
// pass what the primary passes -- which is the pre-batch-45 module exactly.
//
// **The override alone, with no `frame.flags` test ANDed to it, and that is the whole batch.**
// Written the ordinary way -- `SPEC_FLAT_SECONDARY && (frame.flags & FLAG_FLAT_SECONDARY) != 0u`,
// which is what every switch override before batch 41 does -- this is a *run-time* value, so
// the driver cannot prove which arm `shade_hit` takes and keeps **both** the flat lookup and
// the nine-cell gather in each of the three secondary copies it inlines, plus the branch. That
// build was measured: **+1.000 ms at `terraces`, +1.420 at `default`, +1.465 at `lattice`**,
// uniformly slower, and the `default` row is the proof -- a frame with water only at its right
// edge paid the most, so the cost was code existing rather than work running.
//
// The flag is still real and still in `SPEC_MASK`; it keys the pipeline cache, exactly as
// `FLAG_WATER_SHADOW_CUT` and `FLAG_LEAF_THIN` do, and for the reason that entry gives: every
// bit in that mask is fixed for the life of the process, so the key *is* the decision and a
// second copy of it inside the shader can only cost.
fn secondary_smooth() -> bool {
    return !SPEC_FLAT_SECONDARY;
}
// The shadow budget for a surface reached *through* the water is `SPEC_WATER_SHADOW_DIST`,
// declared with the other overrides in `common.wgsl` because batch 41 made it the one number
// a control moves. It was a `const 48.0` here from batch 8 to batch 40; the argument for the
// number, and for why 16 does not cost the sea floor its shadow, is at that declaration.

// How far the one ray through a pane may look, and the only authored number in batch 38b.
//
// **`TRANSMIT` and not `REFRACT`, which is water's word for the same budget.** The ray does not
// bend -- see the transmission in `shade_glass` for why a thin slab must not -- so the name that
// mirrored `WATER_REFRACT_DIST` would have described the one thing this batch corrected.
//
// **192 rather than water's 96, and the reason is what is on the other side.** A refracted
// water ray is looking for a sea floor a few blocks down and is spending its budget inside an
// absorbing medium that has converged to the body colour long before it runs out; a window is
// looking at a *landscape*, and a greenhouse whose far wall dissolves into nothing at
// ninety-six blocks is a worse artefact than the one the cap exists to prevent. It is still a
// cap and not the far plane, because the ray is spent on glass pixels and a window onto open
// terrain would otherwise march the full 1500.
//
// Not swept. It is a reach rather than a look constant -- it decides where the transmitted
// image stops, not what it looks like -- and the batch's own vantage holds nothing 192 blocks
// behind its glass, so a sweep here would have measured zero and said nothing.
const GLASS_TRANSMIT_DIST: f32 = 192.0;
// The shadow budget for a surface reached *through* a pane, and water's argument exactly one
// block later: a transmitted ray aimed near-horizontally under a low sun would otherwise
// grind through hundreds of blocks for a sun term nobody can see through the glass in front
// of it. The number is water's, because the argument is water's.
const GLASS_SHADOW_DIST: f32 = 48.0;

// The sun disc in `sky_base` is 1.5 degrees wide, and a mirror-flat sheet reflects it into
// a spot a pixel or two across, so a calm lake under a low sun would carry no sun at all.
// This is a wide analytic lobe added to a *missed* reflection: no second ray, no divergence
// across the workgroup, and terrain in the mirror direction occludes it for free, because a
// hit replaces the whole term. A wavy normal would give the same glint honestly and take
// the ray coherence with it, which is the trade batch 8c declined.
const GLINT_POWER: f32 = 320.0;
const GLINT_GAIN: f32 = 2.5;
const SUN_TINT: vec3<f32> = vec3<f32>(1.0000, 0.8469, 0.5647);
// The medium's own scattering colour: the same authored value the atlas layer is built
// from, decoded like every other colour since batch 2. Deep water converges to this rather
// than to black, which is what keeps it reading as water and not as a hole in the world.
const WATER_BODY: vec3<f32> = vec3<f32>(0.0100, 0.0947, 0.1474);

// Light available to the medium at a point, as one scalar. Composed the way `shade_hit`
// composes its ambient: half of whichever source dominates there, plus the floor.
//
// **Still one scalar after batch 59, and it is a scope decision rather than an oversight.**
// What this feeds is `water_medium`'s in-scattering, which is multiplied by `WATER_BODY` -- a
// colour of its own -- so a coloured block term here would be two hues multiplied, and the
// water's would win at any depth worth seeing. `light_block_level` is `max(r, g, b)`, which is
// exactly what the pre-59 single channel held at every cell a white emitter reached, so this
// function is bit-exact against that build under both arms of `SPEC_LIGHT_RGB` and needs
// neither. A lamp under water lighting the water its own colour is roadmap A2's, not R4's.
fn medium_light(packed: u32) -> f32 {
    let sky_l = light_curve(light_sky(packed)) * frame.daylight;
    let blk_l = light_curve(light_block_level(packed));
    return max(sky_l, blk_l) * 0.50 + frame.ambient;
}

// ---------------------------------------------------------------------------------------
// Water micro-normals (batch 16)
// ---------------------------------------------------------------------------------------
//
// Four directional cosines over world XZ, summed as *slopes* and never integrated into a
// height. **Nothing is displaced.** The visibility buffer holds one flat voxel face per
// pixel, `hit_t` re-derives the hit point from that face and `taa` reprojects the very same
// point; a displaced surface would have to be traced, stored and reprojected, and there is
// nowhere in the key to put it. So this perturbs shading and nothing else, which is the
// whole reason batch 8's decision can be overturned for the price of a few cosines.
//
// **The waves are the easy half; the band limit is the batch.** This normal is evaluated
// once per pixel with no mip chain under it, and at 600 blocks its finest octave is a
// quarter of a pixel across. Left alone it aliases into a field of white sparkle that the
// temporal pass answers by smearing, and the sea reads as noise rather than as water. So
// each octave fades out as its wavelength approaches the pixel footprint, and the slope
// variance the fade removes is handed to the glint lobe as *roughness* instead of being
// dropped -- Toksvig's trade, done against a footprint this function already had a use for.
//
// The four ratios are irrational-ish so the sum has no period a still frame can read as a
// grid, and the phase rates are `sqrt` of the frequency ratios, which is deep-water
// dispersion: short waves travel slower, so the sum never repeats in time either. That is
// free -- four constants -- and it is the difference between a sea and a scrolling texture.

const WAVE_TAU: f32 = 6.28318531;
// The smallest grazing sine the band limit will divide by, so `kk` stays inside f32 with
// room to spare. 1e-3 is 0.057 degrees off the horizontal -- far past the angle at which
// every octave has already faded out, so it bounds the arithmetic and never the look.
const WAVE_GRAZE_MIN: f32 = 1e-3;
// ---- batch 20: how deep the water has to be to carry the full swell ----
//
// Sheltered shallow water is calm, and this engine's sea was not: a two-block lagoon over
// sand wore the same open-ocean swell as a point 900 blocks out, which is what reads as "too
// much" from the shore. The amplitude ramps from nothing at the waterline to full at
// `SHOAL_DEPTH` blocks of water under the pixel.
//
// This scales the *slope* going in rather than fading octaves out, and the difference
// matters: a band limit removes detail the pixel cannot resolve and hands the variance to
// the glint lobe, because the waves are still there. Here they are genuinely not there, so
// the variance has to go with them -- which it does for free, `wave_octave` deriving it from
// the same `slope` argument.
const SHOAL_DEPTH: f32 = 6.0;
// The ray is exactly as long as the constant it feeds, so a miss means "deeper than the ramp
// cares about" and `smoothstep(0, D, D)` is 1 with no second budget to keep in step.
const SHOAL_MAX: f32 = SHOAL_DEPTH;
// Straight down, tilted by a thousandth, and the tilt is load-bearing. `trace_world` builds
// its chunk-exit times from `1.0 / rd`, so an exactly axis-aligned ray puts `inf` on the two
// dead axes, `t_exit` comes back `-inf`, and the chunk walk steps sideways sixteen times and
// returns a miss. It fails *silently* -- a miss reads as deep water, so the whole feature
// would do nothing and look like a constant that needed tuning. Over `SHOAL_MAX` blocks this
// drifts 0.006 of a block.
const SHOAL_DIR: vec3<f32> = vec3<f32>(0.001, -0.999999, 0.001);
// The width of the Gaussian the footprint is filtered through, as `exp(-K * (ext/lambda)^2)`.
//
// **Derived to 1.645 and shipped at 1.0, and the gap is the interesting part.** A box of
// length `ext` has variance `ext^2 / 12`, and the Gaussian of that variance attenuates a
// cosine by `exp(-(pi^2 / 6) (ext/lambda)^2)` -- so the derivation says `pi^2 / 6`, or
// 1.6449. Swept against an unfiltered ground truth at two vantages, the minimum is at
// **0.9** and the basin is flat from 0.7 to 1.2; 1.645 sits 2.1% up the far side of it.
// 1.0 is the round number inside the basin and scores within 0.001 of the minimum.
//
// The gap has a mechanism and is not noise. Fading an octave takes its slope variance out
// of *every* term the normal feeds, and only one of them gets it back: `gloss` widens the
// sun glint and nothing widens the Fresnel weight or the sky lookup. So a filter tuned to
// the geometry alone over-flattens, and the sweep buys some of that back by under-filtering.
// Spending the variance in the other two terms is what would let this constant be the
// derived one -- see the batch's own file for why that is a separate batch.
const WAVE_FILTER_K: f32 = 1.0;
// Directions, weights and frequency ratios of the four octaves. Unrolled rather than looped
// over a `const` array for batch 12's reason: indexing a table with a runtime value costs a
// per-lookup stack copy, and this pass has no stack today.

const WAVE_D0: vec2<f32> = vec2<f32>( 0.913089,  0.407760);
const WAVE_D1: vec2<f32> = vec2<f32>(-0.388685,  0.921371);
const WAVE_D2: vec2<f32> = vec2<f32>(-0.980382, -0.197108);
const WAVE_D3: vec2<f32> = vec2<f32>( 0.477328, -0.878725);

// ---- batch 25: the fill schedule ----
//
// Eight octaves subdividing the *same* 32..3.14 band the four above span, at the geometric
// ratio `10.20^(1/7)` = 1.393, on the same `lambda^0.536` amplitude law, renormalised to the
// same total slope variance. The sea is exactly as rough as it was; only the spectrum's
// spacing changed, which is why `--wave-amp` and `--wave-scale` still mean what they meant.
//
// **The count is not the variable, and that is the whole finding.** The roadmap asked for
// octaves added past the fourth; those are shorter than 3.14 blocks, batch 19's band limit
// has already faded them to zero at the grazing mid-distance vantage where the repeat is
// reported, and adding them moves the peak autocorrelation by 0.016. Subdividing inside the
// band the filter passes moves it from 0.999 to 0.731. Nor is the amplitude law a way out:
// swept over four exponents, four octaves never score better than 0.979 at any of them, and
// six octaves at their best exponent never reach eight at the shipped one.
//
// The directions are a constrained search -- 20 seeds, best of, no two octaves within 15
// degrees. The constraint is authored and it costs 0.03 to 0.05: the unconstrained optimum
// puts two octaves at the *same* angle, and a batch whose premise is batch 20's "the four are
// really two, crossed" cannot ship that to buy back a hundredth. `tests/waves.rs` re-derives
// every number in this comment from these constants.
const WAVE_F0: vec2<f32> = vec2<f32>(-0.438371,  0.898794);
const WAVE_F1: vec2<f32> = vec2<f32>(-0.927184,  0.374607);
const WAVE_F2: vec2<f32> = vec2<f32>(-0.190809,  0.981627);
const WAVE_F3: vec2<f32> = vec2<f32>(-0.669131,  0.743145);
const WAVE_F4: vec2<f32> = vec2<f32>( 0.121869,  0.992546);
const WAVE_F5: vec2<f32> = vec2<f32>( 0.694658,  0.719340);
const WAVE_F6: vec2<f32> = vec2<f32>( 0.970296,  0.241922);
const WAVE_F7: vec2<f32> = vec2<f32>( 0.438371,  0.898794);

// `xy` is d(height)/d(x, z) -- the part of the slope this pixel can still resolve -- and
// `z` is the slope variance the band limit removed, which is not lost but becomes lobe
// width. A `vec3` and not a two-field struct, which is worth 129 SPIR-V words and nothing
// else: it did **not** account for the 32 bytes of Local Memory Size this batch adds to
// `resolve` -- see `docs/batch-16-waves.md`, which records that number, what has been ruled
// out and why it was not chased here.
fn wave_octave(p: vec2<f32>, dir: vec2<f32>, lambda: f32, slope: f32, phase: f32, px: f32,
               vh: vec2<f32>, kk: f32, aniso: bool) -> vec3<f32> {
    var fade = 0.0;
    if aniso {
        // ---- the footprint (batch 19) ----
        //
        // Against the pixel's footprint **on the water plane, along this octave's own
        // direction**, and not against `px`, which is the pixel's width across the view
        // ray. The plane is horizontal and the ray crosses it at `|rd.y|`, so the footprint
        // there is an ellipse `k = 1 / |rd.y|` times longer along the view than across it.
        // `d` is how much of this octave lies along the view and `kk` is `k*k - 1`, so the
        // ellipse's extent along `dir` is `px * sqrt(d*d*k*k + (1 - d*d))` -- one dot, one
        // fma and one sqrt, and exactly `px` when `kk` is zero.
        //
        // Anisotropic and not one widened `px`, because at a grazing angle the pixel is
        // stretched along the view and not across it: a wave train marching toward the eye
        // is unresolvable while one running left-to-right through the same pixel is
        // perfectly resolved. Blurring both is how a distant sea turns back into glass.
        let d = dot(dir, vh);
        let ext = px * sqrt(1.0 + kk * d * d);
        // ---- the filter ----
        //
        // The band-limited amplitude of a cosine averaged over a box of length `ext` is
        // `sinc(ext / lambda)`, which is exact and unusable: it rings, and it goes negative
        // past its first zero. So the footprint is taken as the Gaussian of the same
        // variance -- a box of width `w` has variance `w*w / 12` -- whose Fourier response
        // is `exp(-2 pi^2 sigma^2 / lambda^2)`, and with `sigma^2 = ext^2 / 12` that is
        // `exp(-(pi^2 / 6) (ext / lambda)^2)`. Monotone, never negative, and within 0.02 of
        // the true sinc everywhere the sinc is still positive.
        //
        // The old `smoothstep(2.0, 4.0, lambda / px)` was a stand-in for this and a
        // defensible one while `px` understated the footprint by up to 16x: it never fired.
        // Against the real footprint it is not defensible -- it reaches zero at two samples
        // per period, which is Nyquist itself, so an octave still carrying 71% of its
        // amplitude was being deleted outright. Measured, that cost more than the aliasing
        // it removed; `docs/batch-19-graze.md` has the table.
        let u = ext / lambda;
        fade = exp(-WAVE_FILTER_K * u * u);
    } else {
        // Pre-batch-19, verbatim, and this is the control. Nyquist against the *pixel* and
        // not against a texel, because there is no mip chain under this signal to have
        // pre-filtered anything. Full weight at four pixels per period, gone at two,
        // `smoothstep` between so no ring of sea shows where the cut is.
        fade = smoothstep(2.0, 4.0, lambda / max(px, 1e-4));
    }
    let g = dir * (slope * fade * cos(WAVE_TAU / lambda * dot(p, dir) + phase));
    // A cosine of peak slope `a` has slope variance `a*a/2` over its period, so what the
    // fade took out is the difference of the two squares. This is the whole of the
    // filtering: an octave does not vanish, it turns into roughness.
    return vec3<f32>(g, 0.5 * slope * slope * (1.0 - fade * fade));
}

fn wave_field(p: vec2<f32>, px: f32, vh: vec2<f32>, kk: f32, aniso: bool, shoal: f32) -> vec3<f32> {
    let l0 = frame.wave_scale;
    // `shoal` is 1.0 in deep water and in every run with the flag off, so the control is the
    // multiply reducing to the identity rather than a second path.
    let a = frame.wave_amp * shoal;
    let ph = frame.time * frame.wave_speed * WAVE_TAU;
    // Batch 25. The two schedules are written out rather than looped, for the reason the
    // four above are unrolled: indexing a table with a runtime value costs a per-lookup
    // stack copy and this pass has no stack. `SPEC_WAVE_FILL` picks one at pipeline-compile
    // time, so the build running the four does not carry the eight.
    if SPEC_WAVE_FILL && (frame.flags & FLAG_WAVE_FILL) != 0u {
        return wave_octave(p, WAVE_F0, l0          , a * 0.3086, ph * 1.0000, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F1, l0 /  1.3934, a * 0.2584, ph * 1.1804, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F2, l0 /  1.9417, a * 0.2163, ph * 1.3934, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F3, l0 /  2.7056, a * 0.1810, ph * 1.6449, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F4, l0 /  3.7700, a * 0.1515, ph * 1.9417, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F5, l0 /  5.2533, a * 0.1269, ph * 2.2920, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F6, l0 /  7.3201, a * 0.1062, ph * 2.7056, px, vh, kk, aniso)
             + wave_octave(p, WAVE_F7, l0 / 10.2000, a * 0.0889, ph * 3.1937, px, vh, kk, aniso);
    }
    return wave_octave(p, WAVE_D0, l0,         a * 0.42, ph * 1.0000, px, vh, kk, aniso)
         + wave_octave(p, WAVE_D1, l0 /  2.17, a * 0.27, ph * 1.4731, px, vh, kk, aniso)
         + wave_octave(p, WAVE_D2, l0 /  4.71, a * 0.19, ph * 2.1703, px, vh, kk, aniso)
         + wave_octave(p, WAVE_D3, l0 / 10.20, a * 0.12, ph * 3.1937, px, vh, kk, aniso);
}

// A slope vector is the horizontal part of the normal over its vertical part, so this is
// the whole conversion and the clamp below is a cone clamp on the same quantity.
fn wave_normal(grad: vec2<f32>) -> vec3<f32> {
    return normalize(vec3<f32>(-grad.x, 1.0, -grad.y));
}

// ---- batch 75, roadmap A3: the shoreline. Two constants per half and deliberately few
// numbers, because every one of these is an authoring decision rather than physics:
// `WET_BAND` says how far above the waterline the sand reads soaked, and `WET_DARK` how
// soaked; `FOAM_DEPTH` says how shallow the column has to be before the sea carries foam,
// and the two mottle thresholds say how ragged it is. **The mottle is the value the
// thresholds read**, a cubic-smooth value noise with the crest and decay phases permuted
// between cells: a crest takes half a cycle anywhere, and at 0.55 cycles a second a
// far facet's shimmer is dominated by the `refract_detail` fade that gates the whole term
// (whose far end is 96, then 32 more to 0 below), so the foam never has to outlive the
// detail fade it is nested inside of.
const WET_BAND: f32 = 1.5;
const WET_DARK: f32 = 0.62;
const FOAM_DEPTH: f32 = 2.5;
// ---- A9 caustics (`--caustics`), three authoring constants and each named: how deep the
// sun still ties sheets to the floor (the gain decays one e-fold per `1/CAUSTIC_FADE`
// blocks), and the filter width the two octaves are read at -- wide enough that at one
// block per texel the crossings do not shimmer, narrow enough that the ribs stay ribs.
// `CAUSTIC_GAIN` is the strength the by-eye pass reads up or down, and is the only one of
// the three that is a taste rather than a fact: measured against this flame test -- the
// pattern must outlive `--anim-rate 0` dephasing and vanish with it, both visibly.
const CAUSTIC_DEPTH: f32 = 8.0;
const CAUSTIC_FADE: f32 = 0.30;
const CAUSTIC_PX: f32 = 0.35;
// |slope|² · |slope|² of two trains with typical slopes O(0.1) tops out around 1e-4, and
// the ribbons are meant to crest near +80% of direct sun at the crossings, not +1%.
const CAUSTIC_GAIN: f32 = 4000.0;
const FOAM_LO: f32 = 0.45;
const FOAM_HI: f32 = 0.75;
const FOAM_MAX: f32 = 0.85;
const FOAM_RATE: f32 = 0.55;
// Slightly blue-lit white: the sky is what lights a surface this exposed, and
// `body`'s light term multiplies it below so a storm-dark sea dawns-dark foam too.
const FOAM_RGB: vec3<f32> = vec3<f32>(0.92, 0.97, 1.0);

// 97c's `--water-look` terms (SPEC_WATER_LOOK). **Murk** is the references' seeded-lake
// body: a green-olive in-scatter that grows with the traced column depth, so deep water
// reads vegetal rather than ink-blue. **Shallow** is the bank band's sunlit turquoise,
// mixed only where the column shallows *and* the refraction leg ran (the foam's own
// gate, for the foam's own reason: a farther pixel never traced a depth to grade by).
// Ripple amplitude tilts `nw` a few degrees at metre scale -- Fresnel shimmer, not swell.
const WATER_LOOK_MURK: vec3<f32> = vec3<f32>(0.82, 0.90, 0.70);
const WATER_LOOK_MURK_DIST: f32 = 7.0;
const WATER_LOOK_SHALLOW: vec3<f32> = vec3<f32>(0.55, 0.74, 0.68);
const WATER_LOOK_SHORE_DEPTH: f32 = 2.0;
const WATER_LOOK_RIPPLE_AMP: f32 = 0.16;

// Two travelling interference waves at metre scale, returned in *gradient* form the way
// `wave_field` returns its slopes -- the fine chop the wave field's band limit
// deliberately removes. Two near-orthanted directions beating against each other, so the
// pattern never reads as one travelling stripe; periods chosen co-prime-ish (1.15 vs
// 0.62) so the beat itself wanders. `time` arrives pre-multiplied by the wave clock, the
// foam's clock rather than the raw one, for the clock-consistency rule batch 81b pinned.
// Since 101d the two train weights are the *caller's*, not fixed here: each train's
// amplitude is fade-shaped against its own wavelength and the grazing-stretched footprint
// (the second round's moire finding), so the caller passes the weighted pair rather than
// a constant 0.60/0.40. Off the arm nothing here runs, as before.
fn look_ripple(p: vec2<f32>, time: f32, w1: f32, w2: f32) -> vec2<f32> {
    let d1 = vec2<f32>(0.86, 0.51);
    let d2 = vec2<f32>(-0.42, 0.91);
    let k1 = 6.2832 / 1.15;
    let k2 = 6.2832 / 0.62;
    var phase = vec2<f32>(0.0);
    if SPEC_WATER_REPAIR {
        // Analytic smooth world-space phase modulation, no voxel/cell hash discontinuities.
        phase = vec2<f32>(sin(dot(p, vec2<f32>(0.071, 0.113))),
                          sin(dot(p, vec2<f32>(-0.097, 0.053)) + 1.9)) * 2.0;
    }
    let g1 = d1 * (cos(dot(p, d1) * k1 + time * 1.7 + phase.x) * w1);
    let g2 = d2 * (cos(dot(p, d2) * k2 - time * 2.3 + phase.y) * w2);
    return g1 + g2;
}

// 2D lattice hash for the foam value noise, on `common.wgsl`'s `hash_u32`: the two
// multipliers are coprime so no axis of cells ever shares a corner value, and the wrap
// around 2^32 is what makes negative coordinates deterministic.
fn foam_hash(i: vec2<f32>) -> f32 {
    let q = vec2<i32>(i);
    return f32((hash_u32(u32(q.x) * 374761393u + u32(q.y) * 668265263u) >> 8u) & 0xFFFFFFu) / 16777216.0;
}

fn foam_mottle(xz: vec2<f32>, time: f32) -> f32 {
    let p = xz * (1.0 / 0.7);
    let i = floor(p);
    let w = fract(p);
    let u = w * w * (3.0 - 2.0 * w);
    let m0_ = mix(foam_hash(i), foam_hash(i + vec2<f32>(1.0, 0.0)), u.x);
    let m1_ = mix(foam_hash(i + vec2<f32>(0.0, 1.0)), foam_hash(i + vec2<f32>(1.0, 1.0)), u.x);
    let m = mix(m0_, m1_, u.y);
    // One period per cell in half a cycle's crest: the sine runs twice the base rate, and
    // each cell starts its own crest a permuted fraction of a period late -- `foam_hash`
    // again rather than a modulo trick -- so the whole line never breathes at once.
    let ph = time * FOAM_RATE * 4.0 + foam_hash(i * vec2<f32>(2.23, 2.71)) * 6.28318;
    return m * (0.5 + 0.5 * sin(ph));
}

// The water surface: a Fresnel mix of a traced reflection against a traced refraction, the
// second absorbed by the medium over the distance it actually travelled through it.
//
// Nothing here treats water as an opaque albedo. The atlas layer is the medium's scattering
// colour -- what the refracted image converges to with depth -- so its mottling reads as
// varying turbidity and averages to a flat teal at the mips distant water picks.
// ---- P12's shared legs (batch 80) ----
//
// These two functions are the *trace* halves of `shade_water`'s secondary legs, lifted
// out with their arithmetic **verbatim** -- every constant, every argument order, the
// miss convention -- because batch 8's rule stands: the refracted and reflected rays
// need the identical shading, and now a *half-resolution* evaluation of the same legs
// (P12) needs it a third time. What did not move is the compositing: the ramps, the
// per-pixel sky on a reflection miss, the medium against *this* pixel's `body`. Those
// are per-pixel terms, and per-pixel is the whole half of the model a shared sample may
// not approximate. The `.a` channels encode `water_refract_leg`'s column depth (for the
// foam, batch 75) and `water_reflect_leg`'s hit-or-miss, which the callers interpret the
// same way the code did before the lift.
/// `under` is **black on a miss**, not skipped: that is exactly the value the compositor
/// computed before the lift, because a refracted ray finding no bottom converges on the
/// medium of an empty column. `.a` is the column depth (`WATER_REFRACT_DIST` on a miss),
/// which batch 75's foam reads. The enable/fade guards stay **at the caller**: they are
/// the reason far water never traces, and sinking them into the callee would trace every
/// pixel the ramp exists to skip.
fn water_refract_leg(hit: vec3<f32>, n: vec3<f32>, rd: vec3<f32>, t: f32) -> vec4<f32> {
    let rdir = refract(rd, n, 1.0 / WATER_IOR);
    let ro2 = hit + rdir * SURFACE_EPS;
    let h2 = trace_world(ro2, rdir, WATER_REFRACT_DIST, true);
    var under = vec3<f32>(0.0);
    var depth = WATER_REFRACT_DIST;
    if h2.hit {
        let p2 = ro2 + rdir * h2.t;
        under = shade_hit(h2.ci, h2.voxel, block_at(h2.ci, h2.voxel), h2.normal, p2, rdir, t + h2.t,
                          SPEC_WATER_SHADOW_DIST, secondary_smooth(), false);
        depth = h2.t;
    }
    return vec4<f32>(under, depth);
}

/// `.a` is 1.0 on a hit and 0.0 on a miss-or-disabled -- the caller keeps its per-pixel
/// sky for the zero case, because a reflection that misses is per-pixel sky and sharing
/// it is the one thing a half-res pass may not do.
fn water_reflect_leg(hit: vec3<f32>, n: vec3<f32>, mirror_t: vec3<f32>, t: f32) -> vec4<f32> {
    // The offset keeps the *flat* normal: it exists to get off the face, and a tilted
    // one can push the origin along the surface instead of away from it.
    let ro3 = hit + n * SURFACE_EPS;
    let h3 = trace_world(ro3, mirror_t, WATER_REFLECT_DIST, true);
    if !h3.hit {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let p3 = ro3 + mirror_t * h3.t;
    // **`frame.shadow_dist`, and not `SPEC_WATER_SHADOW_DIST`, and batch 44 measured
    // the asymmetry rather than inheriting it.** The refracted leg marches 16 blocks
    // because batch 41 found 1.68 ms in shortening it; this one marches the primary
    // budget of 220, and cutting it buys **nothing** -- 220 -> 48 is -0.026 +/- 0.025
    // of `resolve` at `coastline`, 220 -> 16 is -0.044 +/- 0.029, and **deleting the
    // shadow outright is -0.019 +/- 0.056**, every one of them inside its own standard
    // error.
    //
    // **The medium is the reason, and it is the same sentence batch 42 wrote about
    // land.** A refracted ray's hit is *under the sea*, so the shadow ray it casts sets
    // off through water, which is transparent to it, and grinds for hundreds of blocks.
    // A reflected ray's hit is a hillside standing in *air*: its shadow ray either
    // strikes terrain at once or escapes to open sky at once, so there is no long tail
    // to truncate. The budget could be 16 for free -- the band holds **273 pixels of
    // 14,745,600 over the whole vantage set at max channel delta 8** -- and it stays
    // 220 because a control has to buy something, and this one buys zero.
    var lit = shade_hit(h3.ci, h3.voxel, block_at(h3.ci, h3.voxel), h3.normal, p3, mirror_t, t + h3.t,
                        frame.shadow_dist, secondary_smooth(), false);
    // The reflected leg starts at the water surface and not at the eye, which is
    // exactly what `transmittance_from` exists to say. `sky_base`, not `sky_color`:
    // this is a fog target, so it takes the same no-clouds rule the primary one does.
    lit = mix(lit, sky_base(mirror_t, 1.0), 1.0 - transmittance_from(hit.y, h3.t, mirror_t));
    return vec4<f32>(lit, 1.0);
}

// The whole of `shade_water` short of the two traced legs, so the legs can arrive from
// either of two places (batch 81, roadmap P12; see `SPEC_WATER_SEC`). Field order is the
// order of evaluation in the code below; keep both lists in step.
struct WaterCtx {
    body: vec3<f32>,       // tint * medium light, this pixel
    ml: f32,               // the medium's light multiplier, shared with the foam
    n: vec3<f32>,          // flat face normal
    nw: vec3<f32>,         // fully perturbed wave normal
    nt: vec3<f32>,         // slope-capped normal for the traced reflection
    gloss: f32,            // Toksvig lobe width: 1 mirror-flat, band-limited below
    waved: bool,           // waves fired on this surface
    refract_detail: f32,
    reflect_detail: f32,
    mirror: vec3<f32>,     // reflect(rd, nw), for sky and glint
    mirror_t: vec3<f32>,   // the lifted traced direction
    hit: vec3<f32>,
    rd: vec3<f32>,
    t: f32,
    roughness: f32,
}

fn water_ctx(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32) -> WaterCtx {
    let c = chunks[ci];
    let vs = c.voxel_size;
    let n = normal_of(normal_id);
    let box_min = c.origin + vec3<f32>(v) * vs;
    let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));

    let footprint = max(t, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs;
    let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
    // **Stride 8, not 6.** `face_table` emits eight entries per block -- the six faces then
    // face 0 twice for batch 14's two cross-quad normal states -- and this call site was
    // written in batch 8 against the old stride of six and missed when batch 14 widened it.
    // At stride 6 water's +Y face reads index 81, which at the real stride is block 10 face
    // 1: **`tex::GLASS`**, which is how the sea spent batches 14 to 21b wearing a pane's
    // colour. See `docs/batch-21b-water-layer.md`.
    let layer = face_layer(block_faces[WATER_ID * 8u + normal_id]);
    var tint = textureSampleLevel(atlas, atlas_samp, face_uv(normal_id, local), i32(layer), lod).rgb;
    if SPEC_WATER_REPAIR {
        // Coarsest existing mip: the water body is a medium, not a tiled cube decal.
        tint = textureSampleLevel(atlas, atlas_samp, vec2<f32>(0.5), i32(layer), 3.0).rgb;
    }
    // Named rather than inlined for batch 75's foam, which wants the same light the body
    // carries (same multiply, same rounding order).
    let ml = medium_light(light_at(ci, vec3<i32>(v)));
    let body = tint * ml;

    let refract_detail = 1.0 - smoothstep(WATER_REFRACT_NEAR, WATER_REFRACT_FAR, t);
    let reflect_detail = 1.0 - smoothstep(WATER_REFLECT_NEAR, WATER_REFLECT_FAR, t);

    // ---- micro-normals: three normals, because the terms want different ones (16) ----
    //
    // `nw` is the full perturbed normal and it is what Fresnel, the analytic glint and the
    // sky lookup take: none of the three touches the tree, so batch 8's coherence argument
    // never applied to them. `nt` is the same normal with its slope capped, for the one
    // reflected ray that does traverse. The refraction below keeps `n` outright -- it is
    // the more expensive of the two traces and the one whose divergence would be paid on
    // 35% of a sea-filling frame rather than 2%.
    let px_world = max(t, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y);
    var nw = n;
    var nt = n;
    // Toksvig: 1 is the mirror-flat sheet and the batch-8 lobe exactly.
    var gloss = 1.0;
    var unresolved = 0.0;
    let waved = SPEC_WAVES && (frame.flags & FLAG_WAVES) != 0u && normal_id == NORMAL_PY;
    if waved {
        // The whole of batch 19 on this side: the grazing stretch scales the field by the
        // sine of the grazing angle, unbounded near the horizon and *correct* there -- a
        // pixel covers hundreds of blocks of sea and can resolve none of it. What is left
        // of the culled octaves is not lost: `w.z` carries their slope variance into the
        // glint's lobe width -- a properly filtered distant sea is glossy, not glassy.
        // One `if` on the override, not a `let`, for batch 15's reason, checked with
        // `shaderstats` rather than assumed: `docs/batch-19-graze.md`.
        var kk = 0.0;
        var vh = vec2<f32>(0.0, 0.0);
        let aniso = SPEC_WAVE_ANISO && (frame.flags & FLAG_WAVE_ANISO) != 0u;
        if aniso {
            let k = 1.0 / max(abs(rd.y), WAVE_GRAZE_MIN);
            kk = k * k - 1.0;
            vh = rd.xz * inverseSqrt(max(dot(rd.xz, rd.xz), 1e-12));
        }
        // ---- how much swell this column of water can carry (batch 20) ----
        // Sampled at the voxel's centre in XZ and not at the sub-pixel hit point, because
        // what is wanted is the column's depth and not a reading that moves within a face.
        var shoal = 1.0;
        if SPEC_WAVE_SHOAL && (frame.flags & FLAG_WAVE_SHOAL) != 0u {
            let dro = vec3<f32>(floor(hit.x) + 0.5, hit.y - SURFACE_EPS, floor(hit.z) + 0.5);
            let dh = trace_world(dro, SHOAL_DIR, SHOAL_MAX, true);
            var depth = SHOAL_MAX;
            if dh.hit {
                depth = dh.t;
            }
            shoal = smoothstep(0.0, SHOAL_DEPTH, depth);
        }
        let w = wave_field(hit.xz, px_world, vh, kk, aniso, shoal);
        unresolved = max(w.z,0.0);
        nw = wave_normal(w.xy);
        // A cone clamp, and no trig: the slope vector's length *is* the tangent of the
        // tilt, so scaling it down is exactly clamping the angle, and small perturbations
        // pass through untouched rather than being scaled.
        nt = wave_normal(w.xy * min(1.0, frame.wave_reflect_slope / max(length(w.xy), 1e-6)));
        // The variance the band limit removed, spent as lobe width: a Phong exponent `p`
        // is about `2 / sigma2` of slope variance, and the gain follows the exponent, so
        // the lobe's *integral* stays put -- a wider glitter path is not a brighter one.
        gloss = 1.0 / (1.0 + 0.5 * GLINT_POWER * w.z);
    }

    // 97c (`--water-look`): the references' water is never optically flat -- fine
    // animated interference ripples sit on *top of* whatever swell the wave field gave,
    // and under no wave flag at all. **`nw` alone** (Fresnel, glint, sky): the traced
    // normal `nt` never moves, so the arm cannot move a march's cost class -- the wave
    // field's own cap argument, made once more and priced as happily. `+Y` faces only:
    // a water side face carrying ripple shimmer is a pane pretending to be choppy.
    if SPEC_WATER_LOOK && normal_id == NORMAL_PY {
        // 101d: the second round found this arm's one weakness, and the wave field
        // already owns its cure. One amplitude at every range could only read as
        // invisible or moire -- the sheet's aerial ring fields and the near-band
        // chevrons alike (and the *pre-existing* static field carries its banding past
        // the same ranges, so there is no old-and-innocent to retreat to). `wave_octave`
        // folds its band limit into gloss against the grazing-stretched footprint; this
        // term takes the same Nyquist class a level down: `pix` is the isotropic
        // footprint stretched by the wave field's own grazing factor, and each wave
        // train is full weight at four samples per period and gone at two -- the exact
        // cut the control-side octaves receive against `px`, so the short train (the
        // chevron and ring maker) dies first and the ripple can never outlive the
        // filtered swell beside it.
        let pix = px_world / max(abs(rd.y), WAVE_GRAZE_MIN);
        let range_gate = (1.0-smoothstep(48.0,96.0,t))*smoothstep(0.08,0.20,abs(rd.y));
        let fade_115 = range_gate*smoothstep(2.0, 4.0, 1.15 / max(pix, 1e-4));
        let fade_62 = range_gate*smoothstep(2.0, 4.0, 0.62 / max(pix, 1e-4));
        unresolved += WATER_LOOK_RIPPLE_AMP*WATER_LOOK_RIPPLE_AMP*0.5*
            (0.36*(1.0-fade_115*fade_115)+0.16*(1.0-fade_62*fade_62));
        let rp = look_ripple(
            hit.xz,
            frame.time * frame.wave_speed,
            0.60 * fade_115,
            0.40 * fade_62,
        );
        nw = normalize(nw + vec3<f32>(rp.x, 0.0, rp.y) * WATER_LOOK_RIPPLE_AMP);
    }

    // Fade unresolved slopes for Fresnel, sky and traced mirror directions together.
    let optical_roughness=clamp(1.0-exp(-8.0*unresolved),0.0,1.0);
    nw=normalize(mix(nw,n,optical_roughness));
    nt=normalize(mix(nt,n,optical_roughness));
    // ---- reflection direction, as batch 8 split it ----
    // The lifted traced direction: a facet steep enough to reflect the view ray *into*
    // the sea is one its own neighbours mask, and a ray traced down there comes back with
    // the sea floor in it as though that were sky. The lift sits inside `waved` because
    // `normalize` of an already-unit vector is not the identity in floating point, and a
    // control has to be bit-exact rather than merely equivalent.
    let mirror = reflect(rd, nw);
    var mirror_t = reflect(rd, nt);
    if waved {
        mirror_t = normalize(vec3<f32>(mirror_t.x, max(mirror_t.y, 0.0), mirror_t.z));
    }

    return WaterCtx(body, ml, n, nw, nt, gloss, waved, refract_detail, reflect_detail,
                    mirror, mirror_t, hit, rd, t, optical_roughness);
}

// Everything after the legs: the per-pixel compositing a shared sample may NOT borrow.
// The leg values themselves -- `water_refract_leg`'s `(under, depth)` and
// `water_reflect_leg`'s `(lit, 1|0)` -- are the only input the two arms differ in.
fn water_compose(ctx: WaterCtx, leg_r: vec4<f32>, leg_m: vec4<f32>) -> vec3<f32> {
    // ---- refraction (8b): the ramp and the medium are this pixel's ----
    var refr = ctx.body;
    var column_depth = WATER_REFRACT_DIST;
    if (frame.flags & FLAG_WATER_REFRACT) != 0u && ctx.refract_detail > 0.0 {
        refr = mix(refr, water_medium(leg_r.rgb, leg_r.a, ctx.body), ctx.refract_detail);
        column_depth = leg_r.a;
    }

    // 97c (`--water-look`): hue graded by the column the refraction leg already paid
    // to trace -- the references' seeded-lake murk in depth, sunlit turquoise at the
    // bank -- riding the foam's own gate, so a pixel without a traced depth is by
    // definition too far to show either term. Albedo-shaped (it multiplies and mixes
    // `refr` before Fresnel weighs it), so the medium is a material here, not a light.
    if SPEC_WATER_LOOK && ctx.refract_detail > 0.0 {
        let depth_f = clamp(column_depth / WATER_LOOK_MURK_DIST, 0.0, 1.0);
        refr = refr * mix(vec3<f32>(1.0), WATER_LOOK_MURK, depth_f);
        let shall = 1.0 - smoothstep(0.0, WATER_LOOK_SHORE_DEPTH, column_depth);
        refr = mix(refr, WATER_LOOK_SHALLOW * ctx.ml, shall * 0.45 * ctx.refract_detail);
    }

    // ---- foam: A3's water half, a term on `column_depth` and nothing else ----
    if SPEC_SHORE_FOAM && ctx.refract_detail > 0.0 && column_depth < FOAM_DEPTH {
        let shall = 1.0 - smoothstep(0.0, FOAM_DEPTH, column_depth);
        // The wave clock, not the raw one: `--anim-time` pins `time` only through its
        // two speed channels, and an ocean-surface term belongs to the wave channel.
        // (Batch 81b's repair of the third reader the test `textures.rs` counts.)
        let m = foam_mottle(ctx.hit.xz, frame.time * frame.wave_speed);
        let foam = shall * smoothstep(FOAM_LO, FOAM_HI, m) * ctx.refract_detail;
        refr = mix(refr, FOAM_RGB * ctx.ml, clamp(foam, 0.0, 1.0) * FOAM_MAX);
    }

    // ---- reflection (8c): base sky and glint are this pixel's; the trace is shared ----
    // The reflected ray's base sky reads from the surface, not the eye: the cloud deck is
    // at a finite altitude and the camera's origin would give the whole sea one
    // parallax-free sky sliding with the camera. **1.0, not a shaft** -- batch 10's
    // lesson is to check who else calls the function: `water_compose` evaluates the sky
    // unconditionally, so a shaft here would put eight shadow rays on every water pixel
    // in the frame to shadow a lobe the Fresnel weight then throws most of away.
    var refl = sky_color(ctx.hit, ctx.mirror, ctx.t, 1.0)
        + SUN_TINT * (pow(max(dot(ctx.mirror, frame.sun_dir), 0.0), GLINT_POWER * ctx.gloss)
                      * GLINT_GAIN * ctx.gloss * frame.daylight);
    if (frame.flags & FLAG_WATER_REFLECT) != 0u && ctx.reflect_detail > 0.0 && leg_m.a > 0.0 {
        refl = mix(refl, leg_m.rgb, ctx.reflect_detail);
    }

    if SURFACE_DEBUG == 5u { return ctx.n * 0.5 + vec3<f32>(0.5); }
    if SURFACE_DEBUG == 6u { return refl; }
    if SURFACE_DEBUG == 7u { return refr; }
    // Approximate prefilter for unresolved reflection detail, including the traced leg.
    // No extra rays. This is a bounded rough-sky blend, not an exact BRDF convolution.
    refl=mix(refl,sky_base(ctx.n,1.0),ctx.roughness*0.65);
    // Fresnel off the perturbed normal, most of what the eye reads as "water".
    return mix(refr, refl, schlick(max(-dot(ctx.rd, ctx.nw), 1e-3)));
}

fn shade_water(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32) -> vec3<f32> {
    let ctx = water_ctx(ci, v, normal_id, hit, rd, t);
    // The enable/fade guards stay *caller* of the trace, for the reason they existed:
    // far water never traces.
    var leg_r = vec4<f32>(0.0, 0.0, 0.0, WATER_REFRACT_DIST);
    if (frame.flags & FLAG_WATER_REFRACT) != 0u && ctx.refract_detail > 0.0 {
        leg_r = water_refract_leg(hit, ctx.n, rd, t);
    }
    var leg_m = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (frame.flags & FLAG_WATER_REFLECT) != 0u && ctx.reflect_detail > 0.0 {
        leg_m = water_reflect_leg(hit, ctx.n, ctx.mirror_t, t);
    }
    return water_compose(ctx, leg_r, leg_m);
}

// Batch 81 (roadmap P12), the **diagnostic arm**: the same compositing, fed the leg
// values a `water_sec` pass wrote for this pixel's owner half-texel. Dumb point sample
// by design -- no guided filter, no edge stop -- because the run that reads it answers
// "*spatial* sharing of these legs, yes or no" against `--reference`, and every ounce of
// good filtering in the arm would be an answer the question didn't ask. On a texel
// whose owner's gate was closed (or held no water) the buffer carries exactly the
// values this arm's own `var` initialisations carry, so the fallback reads through them.
fn water_share_safe(px: u32, py: u32, normal_id: u32, hit: vec3<f32>, t: f32) -> bool {
    if normal_id != NORMAL_PY { return false; }
    let span = 1u << WATER_SEC_SHIFT;
    let base = vec2<u32>((px >> WATER_SEC_SHIFT) * span, (py >> WATER_SEC_SHIFT) * span);
    for (var y=0u; y<span; y++) { for (var x=0u; x<span; x++) {
        let p=base+vec2<u32>(x,y);
        if any(p >= frame.res) { continue; }
        let key=atomicLoad(&vis[p.y*frame.res.x+p.x]);
        if key==0lu { return false; }
        let nid=u32(key>>37u)&7u;
        if nid!=normal_id { return false; }
        let ci=u32(key>>24u)&0x1FFFu;
        let low=u32(key&0xFFFFFFlu);
        let v=vec3<u32>((low>>16u)&VOXEL_MASK,(low>>8u)&VOXEL_MASK,low&VOXEL_MASK);
        if block_at(ci,v)!=WATER_ID { return false; }
        let micro=vec3<u32>((low>>22u)&3u,(low>>14u)&3u,(low>>6u)&3u);
        let rd=ray_dir(f32(p.x)+0.5,f32(p.y)+0.5);
        let td=hit_t(ci,v,micro,nid,rd);
        let hd=frame.cam_pos+rd*td;
        if abs(hd.y-hit.y)>0.01 || abs(td-t)>max(0.1,min(1.0,t*0.02)) { return false; }
    }}
    return true;
}

fn shade_water_sec(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32,
                   px: u32, py: u32) -> vec3<f32> {
    if SPEC_WATER_REPAIR && !water_share_safe(px, py, normal_id, hit, t) {
        return shade_water(ci, v, normal_id, hit, rd, t);
    }
    let ctx = water_ctx(ci, v, normal_id, hit, rd, t);
    var leg_r = vec4<f32>(0.0, 0.0, 0.0, WATER_REFRACT_DIST);
    var leg_m = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    let hr = vec2<i32>(i32(px >> WATER_SEC_SHIFT), i32(py >> WATER_SEC_SHIFT));
    leg_r = textureLoad(water_refr_sample, hr, 0);
    leg_m = textureLoad(water_refl_sample, hr, 0);
    return water_compose(ctx, leg_r, leg_m);
}

// A pane of glass: what is behind it, what is reflected off it, and Fresnel between the two.
//
// **This is `shade_water` with the medium taken out**, and that is the whole design rather
// than a family resemblance. Water and glass are the same optics -- a surface the primary ray
// stops at, one refracted ray for the content behind it, a Fresnel weight toward a reflection
// -- differing in the index, in `F0`, and in whether the transmitted leg travels through an
// absorbing body. Glass is a pane rather than an ocean, so there is no body: no Beer-Lambert,
// no `water_medium`, no depth term, and the transmitted colour is the hit behind the pane
// multiplied by the pane's own texel. That absence is what makes this function short.
//
// **Three things it deliberately does not do**, each because a batch that did them would be
// measuring more than one change:
//
// - *No traced reflection.* The Fresnel weight goes to `sky_color` alone, which is batch 8's
//   own split -- refraction was 8b and the traced reflection 8c, and they were separated for
//   the same reason. A pane facing a hillside therefore shows sky where it should show hill,
//   most visibly at grazing angles where Fresnel is largest. `--no-water-reflect`'s entry in
//   the control table is the shape a later batch would give this.
// - *No lit pane.* Neither term here is an albedo under a light, so no gather, no shadow ray
//   and no `light_at` -- both the reflection and the transmission carry their own radiance.
//   A pane in shadow and a pane in sun differ only through what each shows.
// - *No second interface.* The transmitted ray ignores every pane it meets, so a greenhouse
//   resolves to what is behind *both* walls. See `march_chunk`'s `skip_glass`.
fn shade_glass(ci: u32, v: vec3<u32>, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, t: f32) -> vec3<f32> {
    let c = chunks[ci];
    let vs = c.voxel_size;
    var n = normal_of(normal_id);
    if dot(n,rd)>0.0 { n=-n; }
    let box_min = c.origin + vec3<f32>(v) * vs;
    let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));

    let footprint = max(t, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs;
    let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
    // **Stride eight, through `face_layer`** -- both halves of that are invariants with a
    // batch behind them, and this call site is the newest place to get either wrong. It is
    // also the layer that batch 21b's bad stride was accidentally *reaching*: water's +Y face
    // at a stride of six lands on block 10 face 1, which is this exact texture, and its
    // `(x + y) % 9` streaks were the salmon marks on the sea floor. Here they are what they
    // were drawn to be -- the highlights on a pane.
    let layer = face_layer(block_faces[GLASS_ID * 8u + normal_id]);
    let texel = textureSampleLevel(atlas, atlas_samp, face_uv(normal_id, local), i32(layer), lod);
    // **The rgb is a transmission tint and the alpha is not read at all.** For every other
    // block that alpha is batch 12's tint mask, and for a tuft it is opacity; for a pane it
    // would be a third meaning, and the one thing this texture must not do is take a biome's
    // colour -- a window is the same window in a tundra and a jungle. The pale border and the
    // diagonal streaks are the only thing that makes an otherwise invisible surface read as a
    // pane at all, and they arrive through this multiply.
    let tint = texel.rgb;
    // **Not permuted.** `block::PERM_CLASS` leaves `tex::GLASS` at `perm::NONE`, so
    // `permute_uv` would be the identity -- but the reason it is not called is the border: a
    // permutation of a bounded, framed tile is exactly the case `perm::NONE` exists for, and
    // turning one would put a pane's edge highlight through its middle.

    // ---- transmission: what is behind the pane ----
    //
    // **The ray does not bend, and that is a correction the batch made to its own design.**
    // The first version called `refract(rd, n, 1.0 / GLASS_IOR)`, copying `shade_water` line
    // for line, and it put a **double image** of everything inside a glasshouse on the screen:
    // looking into a corner, the plinth appeared once through each wall, displaced symmetrically
    // outward. The mechanism is that a slab refracts **twice** -- once going in and once coming
    // out -- and the two very nearly cancel, leaving a lateral offset of a fraction of a block
    // for a pane one block thick. `skip_glass` means this ray never finds the exit face, so only
    // the entry half was ever applied and the bend was never undone.
    //
    // So the honest model for a thin slab is a ray that carries straight on, and it is *more*
    // physical than the bend it replaces rather than a simplification of it. **This is the one
    // place the water model does not generalize**, and the medium is exactly why: a refracted
    // water ray really does continue inside the water, all the way to the sea floor, so its
    // single refraction is the whole story. A pane has a far side four inches away.
    //
    // What still separates glass from a hole in the wall is the Fresnel weight below and the
    // texel tint above, which is what the picture shows and what `GLASS_F0` is for.
    let rdir = rd;
    let ro2 = hit + rdir * SURFACE_EPS;
    // **`skip_water = false`, and it is the whole of the defect this function shipped with.**
    // Every other caller of `trace_world` passes `true` and is right to: water's own refraction
    // starts *inside* the medium, its reflection wants the world above the surface, and a
    // shadow ray must not be stopped by a sea. A ray that has come through a pane is in none of
    // those situations -- it starts in air and the sea is ordinary geometry in front of it -- so
    // with the old hard-coded `true` the surface was simply not there and a window onto a bay
    // came back with the unlit seabed.
    let h2 = trace_world(ro2, rdir, max(GLASS_TRANSMIT_DIST,frame.far), false);
    // A miss is the sky, and it is exact at any distance -- which is why this needs no
    // near/far ramp of the kind `WATER_REFRACT_NEAR` gives the sea. What fades with range
    // there is the *hit*; here there is nothing behind the pane to fade toward.
    var through = sky_color(ro2, rdir, t, 1.0);
    if h2.hit {
        let p2 = ro2 + rdir * h2.t;
        let id2 = block_at(h2.ci, h2.voxel);
        // **The same dispatch the primary ray makes, and for the invariant's own reason**: a
        // surface shaded by a cheaper model through a window reads as a different material than
        // the one beside it. Stopping at water and then handing it to `shade_hit` would draw
        // the sea as a flat blue cube face -- which is *worse* than the bug above, because it
        // looks deliberate. `shade_water` calls `trace_world` twice more, so a glass pixel
        // looking at the sea spends three secondary rays; that is bounded, paid only where a
        // pane actually covers water, and cheaper than any way of faking it.
        //
        // Legal rather than recursive: `shade_water` reaches `trace_world` and `shade_hit` and
        // never comes back here. There is no third transmissive material, and a fourth would
        // have to check that again.
        if id2 == WATER_ID {
            // Water seen through glass: shade as real water with waves, reflections and seabed refraction.
            // Preserves WATER_BODY and schlick( Fresnel from shade_water.
            through = shade_water(h2.ci, h2.voxel, h2.normal, p2, rdir, t + h2.t);
        } else {
            through = shade_hit(h2.ci, h2.voxel, id2, h2.normal, p2, rdir,
                                t + h2.t, GLASS_SHADOW_DIST, secondary_smooth(), false);
        }
        // The haze over the transmitted leg, which starts at the pane and not at the eye --
        // `transmittance_from` is exactly that statement, and it is the same line water's
        // reflected leg takes. Without it a window onto a distant ridge shows that ridge
        // unhazed while the wall beside it is hazed, and the seam is at the frame of the
        // window. `sky_base`, not `sky_color`: a fog target takes the no-clouds rule.
        through = mix(through, sky_base(rdir, 1.0), 1.0 - transmittance_from(hit.y, h2.t, rdir));
    }

    // ---- reflection: the sky off the pane, or (A4, `--glass-reflect`) the world ----
    let mirror = reflect(rd, n);
    var refl = sky_color(hit, mirror, t, 1.0);
    // Batch 90, roadmap A4: trace what the pane actually returns. `water_reflect_leg` is
    // the shape the roadmap prescribes -- one march + one `shade_hit`, generic in all but
    // name, legal here by the recursion note above because the leg never comes back to
    // either transmissive shader. On a miss the leg's `w` is 0 and the sky lookup the arm
    // already computed stands, so the fallback row of a high window never sees a hole.
    // Flagged off by default: whether the world beats the assumption is the by-eye read.
    if SPEC_GLASS_REFLECT {
        let origin=hit+n*SURFACE_EPS;
        // A pane looking at water must see its interface, not the seabed as a mirror.
        let reflected=trace_world(origin,mirror,max(GLASS_TRANSMIT_DIST,frame.far),false);
        if reflected.hit {
            let point=origin+mirror*reflected.t;
            let material=block_at(reflected.ci,reflected.voxel);
            if material==WATER_ID {
                refl=shade_water(reflected.ci,reflected.voxel,reflected.normal,point,mirror,t+reflected.t);
            } else {
                refl=shade_hit(reflected.ci,reflected.voxel,material,reflected.normal,point,mirror,
                    t+reflected.t,GLASS_SHADOW_DIST,secondary_smooth(),false);
            }
            refl=mix(refl,sky_base(mirror,1.0),1.0-transmittance_from(hit.y,reflected.t,mirror));
        }
    }

    // Fresnel off the flat face. `1e-3` rather than 0 for `schlick`'s own reason: the term is
    // clamped there already, and at grazing it has saturated to 1, so the clamp *is* the
    // masking term and it stays continuous across the silhouette of a pane seen edge-on.
    return mix(through * tint, refl, schlick_glass(max(-dot(rd, n), 1e-3)));
}

// What the water converges to when the eye is *inside* it. One grid walk at an address
// every lane in the frame shares, and it is what makes depth darken: the sky-light flood
// has already lost a level per block of water on the way down to the camera.
fn underwater_body() -> vec3<f32> {
    return WATER_BODY * medium_light(world_sample(frame.cam_pos).x);
}

// The water surface seen from **underneath** (batch 54).
//
// **The defect this closes was reported from play, not from a metric.** A player 32 blocks
// down, looking a few degrees above the horizontal, saw a mountain: *"mountain is still visible
// when deep inside water at this i dont think it should be"*. They were right, and until this
// batch nothing here modelled the surface from below at all -- `FLAG_UNDERWATER` makes water
// non-blocking for the primary march, so a submerged ray crossed the boundary as though it were
// not there, and a miss went straight to `sky_color` while a hit above the waterline was shaded
// in full.
//
// **The physics is one angle.** Water's critical angle is `asin(1 / 1.333)` = 48.6 degrees from
// the normal, so a submerged eye sees the world above the surface only inside a cone **41.4
// degrees above horizontal** -- Snell's window. Outside it the surface is a **mirror** and what
// it shows is the sea floor. Nothing about that is a tunable: `WATER_IOR` decides it, and the
// same constant already feeds `refract` in `shade_water`.
//
// **Why the reach could never have fixed this**, which is the other half of the report. A
// `tile_select` clamp fires only when *every* ray in a tile is still under water at the clamp
// distance, and a ray aimed at a peak that breaks the surface leaves the water: the escape
// boundary is `atan(depth / WATER_FAR_DIST)`, 14.0 degrees at that player's depth. Above that
// line a submerged ray is never clamped, and clamping it would delete the **sky** along with the
// mountain -- which is exactly the failure batch 48 was written to avoid. The visible band runs
// from `atan(depth / 128)` up to 41.4 degrees, and its lower edge climbs with depth while its
// upper edge does not, so the deeper the eye the more of the frame was wrong. That is why the
// complaint is about being *deep* and why batch 53, which made deep water cheaper, could not
// make it look right.
//
// **What this does not do, and the reason is architectural rather than a scope cut.** Inside the
// window the world above should be *compressed* into the cone -- the transmitted ray bends at the
// boundary. Bending it is impossible here: `march` has already traced a straight line and put
// its hit in the visibility buffer, so `resolve` cannot change which geometry the primary ray
// found. What is left undone is therefore a **distortion inside the window** and never a
// presence or absence, because outside the window this returns weight 1 to the mirror and the
// transmitted leg is not consulted at all. `docs/water.md` carries it.
fn surface_from_below(above: vec3<f32>, rd: vec3<f32>, dist: f32) -> vec3<f32> {
    // **The control gates the whole term and not merely the traced leg, and the first cut of
    // this got that wrong.** With the flag around the trace alone, `--no-snell` still applied
    // the Fresnel mix against a flat body colour -- so the control reverted the *detail* of the
    // mirror and not the mirror, the frame moved by a max channel delta of 3 where it should
    // have moved by 90, and a `bitexact` row against the parent would have failed for a reason
    // no image would have explained. A control has to reproduce the build it names.
    if (frame.flags & FLAG_SNELL) == 0u {
        return above;
    }
    // **How much of a surface this eye has, which is the user's own correction to the first
    // build.** Zero in the top `SNELL_MIN_DEPTH` blocks, one where the sky flood has already
    // run out. Tested before anything else because it is *uniform across the frame* -- every
    // pixel of a shallow camera takes this branch together, so the whole term costs a compare.
    //
    // At `s == 0` the two `mix`es at the end return `above` bit-exactly (`a * 1 + b * 0` is `a`
    // for any finite `b`), which is why a three-block camera is **identical** to the build
    // without this feature rather than merely close to it -- and why `underwater` and
    // `sea-horizon` are bit-exact rows in the batch's sweep while `deep-water` is not.
    let depth = frame.sea_level - frame.cam_pos.y;
    let s = smoothstep(SNELL_MIN_DEPTH, WATER_DARK_DEPTH, depth);
    if s <= 0.0 {
        return above;
    }
    // A ray heading level or down never reaches the surface, which is the whole lower half of a
    // submerged frame and the cheapest possible early-out.
    if rd.y <= 0.0 {
        return above;
    }
    let t_surf = (frame.sea_level - frame.cam_pos.y) / rd.y;
    // Something under the water is in the way, so the surface is not what this pixel sees.
    // `water_path` makes the same test one line differently and they must agree: a pixel that
    // took the mirror here and the full submerged path there would be absorbed twice.
    if t_surf >= dist || t_surf <= 0.0 {
        return above;
    }
    let surf = frame.cam_pos + rd * t_surf;

    // **How much of this facet is worth resolving.** With the mirror reduced to the body
    // colour this gates one thing -- the wave normal -- and it is still worth having.
    //
    // The first build traced and shaded a mirrored ray for every pixel whose view ray crossed
    // the surface. At a level camera 24 blocks down that is the whole upper half of the frame,
    // and the crossing is *far* for exactly the pixels that dominate it: `t_surf` is
    // `depth / sin(elevation)`, so a ray one degree above the horizontal meets the surface
    // 1,375 blocks away. **Everything traced out there is then absorbed to the body colour on
    // the way back** -- blue's transmittance over 275 blocks is 0.004 -- so the expensive answer
    // and the free one agree to well under a code value.
    //
    // The same near/far ramp the refracted leg uses, on the same quantity it measures: how far
    // you can see through this medium. Beyond it the mirror is `underwater_body()` outright,
    // which is what the trace was converging to anyway.
    //
    // Measured at `deep-water`, `resolve`, against the build without the feature: the wave
    // normal is **1.12 ms**, the trace **2.50**, and shading what it found **2.87**. The last
    // two are gone; this gate stands in front of the first.
    let mirror_detail = 1.0 - smoothstep(WATER_REFRACT_NEAR, WATER_REFRACT_FAR, t_surf);

    // ---- the normal of the facet the ray meets ----
    //
    // The same field `shade_water` reads, at the same band limit and with the same grazing
    // stretch, because it is the same surface -- only the side has changed. **`px_world` is
    // measured to the crossing and not to the hit**: the facet is at `t_surf`, and a pixel
    // aimed past it at a mountain a kilometre away would otherwise ask the band limit to filter
    // the sea over a footprint belonging to the mountain.
    var nup = vec3<f32>(0.0, 1.0, 0.0);
    // The band limit has already flattened a facet this far off -- `px_world` there is hundreds
    // of blocks wide -- so past the fade the perturbed normal and the flat one are the same
    // normal, and this skips eight octaves to say so.
    if SPEC_WAVES && (frame.flags & FLAG_WAVES) != 0u && mirror_detail > 0.0 {
        let px_world = max(t_surf, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y);
        var kk = 0.0;
        var vh = vec2<f32>(0.0, 0.0);
        let aniso = SPEC_WAVE_ANISO && (frame.flags & FLAG_WAVE_ANISO) != 0u;
        if aniso {
            let k = 1.0 / max(abs(rd.y), WAVE_GRAZE_MIN);
            kk = k * k - 1.0;
            vh = rd.xz * inverseSqrt(max(dot(rd.xz, rd.xz), 1e-12));
        }
        // **Shoal is 1 here and that is a stated difference, not an oversight.** Above water
        // `shade_water` spends a trace finding how deep the column under the facet is, so a
        // two-block lagoon carries less swell than open sea. From below, the eye is *in* that
        // column and already knows it is deep enough to be swimming in; buying the same answer
        // would cost a second ray to modulate a normal that feeds one Fresnel term and one
        // mirror direction. `--no-wave-shoal` therefore does nothing to this term, which is
        // what that control's own value already is.
        let w = wave_field(surf.xz, px_world, vh, kk, aniso, 1.0);
        nup = wave_normal(w.xy);
    }

    // ---- Fresnel, with total internal reflection falling out of it ----
    //
    // **Schlick is written against the *transmitted* angle, and that is what makes it correct
    // going the other way.** The usual form takes the incident cosine and is only right when the
    // ray enters the denser medium; leaving one, the reflectance climbs to 1 at the critical
    // angle and stays there, and no amount of the incident cosine will say so. Snell gives
    // `sin(t) = WATER_IOR * sin(i)`, so `sin^2(t) >= 1` **is** the total-internal-reflection
    // test -- the same line produces the cone and the curve inside it, with one `sqrt` and no
    // trigonometry and no angle constant anywhere.
    let cos_i = clamp(dot(rd, nup), 1e-3, 1.0);
    let sin2_t = WATER_IOR * WATER_IOR * (1.0 - cos_i * cos_i);
    var f = 1.0;
    if sin2_t < 1.0 {
        f = schlick(sqrt(1.0 - sin2_t));
    }

    // ---- what the mirror shows ----
    //
    // `underwater_body()` is the floor of this and the right limit rather than a fallback: a
    // mirrored ray that finds nothing is looking down an unbounded column of water, and that is
    // what an unbounded column of water looks like.
    // **The mirror is the body colour, and the traced version was measured and thrown away.**
    //
    // The first build traced a ray down into the water from the crossing point and shaded what
    // it found. It is the obvious thing and it is **invisible**: against a mirror that is simply
    // `underwater_body()`, the traced one differs by a max channel delta of **3 at a level
    // camera, 3 pitched up 20 degrees and 7 at 45**, and it cost **4 ms of `resolve` at
    // `deep-water`** -- a third of the frame -- because it runs on every pixel whose view ray
    // crosses the surface.
    //
    // The reason it is invisible is the reason it should never have been built: a mirrored ray
    // at these angles travels a long way through water, and `water_medium` converges it to
    // `underwater_body()` on the way back. **The expensive answer and the free one agree because
    // the free one is the limit the expensive one was walking towards.** `errors.md` carries the
    // numbers, and `lessons.md` carries the general form: before making an expensive block
    // cheaper, check whether something already computes what it converges to.
    let below = underwater_body();
    let transmitted = mix(above, underwater_body(), s * SNELL_DIM_MAX);
    return mix(transmitted, below, f * s);
}

// 16x8, not 8x8, and the reason is in PERF.md: this pass holds no `var<workgroup>` and no
// barrier, so its shape is free to change and cannot change its arithmetic. 8x8 is two warps
// per workgroup and Ampere caps an SM at 16 resident workgroups, which puts a ceiling of 32
// warps on the shape alone; 16x8 raises that ceiling to the hardware's own 48. Measured, the
// register count is what actually binds and neither shape reaches its ceiling -- see the
// occupancy table -- so this is the ceiling being lifted off, not a speedup.
@compute @workgroup_size(16, 8, 1)
fn resolve(
@builtin(global_invocation_id) gid: vec3<u32>) {
    let px = gid.x;
    let py = gid.y;
    if px >= frame.res.x || py >= frame.res.y {
        return;
    }
    let pix = py * frame.res.x + px;
    let rd = ray_dir(f32(px) + 0.5, f32(py) + 0.5);

    if RESOLVE_LANE != 0u && (frame.flags & FLAG_HEATMAP) != 0u { return; }
    if (frame.flags & FLAG_HEATMAP) != 0u {
        let it = f32(atomicLoad(&dbg[pix]));
        textureStore(out_tex, vec2<i32>(i32(px), i32(py)), vec4<f32>(heat(it / 300.0), 1.0));
        return;
    }

    let underwater = (frame.flags & FLAG_UNDERWATER) != 0u;
    let key = atomicLoad(&vis[pix]);
    if RESOLVE_LANE != 0u {
        if key == 0lu { return; }
        let filter_ci = u32(key >> 24u) & 0x1FFFu;
        let filter_low = u32(key & 0xFFFFFFlu);
        let filter_v = vec3<u32>((filter_low >> 16u)&VOXEL_MASK,(filter_low >> 8u)&VOXEL_MASK,filter_low&VOXEL_MASK);
        let filter_id = block_at(filter_ci,filter_v);
        if RESOLVE_LANE == 1u && (!SPEC_GLASS || filter_id != GLASS_ID) { return; }
        if RESOLVE_LANE == 2u && filter_id != WATER_ID { return; }
    }
    var color: vec3<f32>;
    var dist = frame.far;
    if key == 0lu {
        // A ray that reaches the sky is entirely in-scattered light, so the shaft's
        // importance test gets an alpha of 1: there is no surface radiance here for the
        // haze to be a fraction of. `frame.far` lets `sun_shaft` apply its own
        // `godray_dist` bound rather than having two places decide the reach.
        //
        // Underwater the shaft is 1.0 and the reason is the medium: this integral is the
        // *air's* single-scattering, and a shaft through water wants water's own phase
        // function and extinction, which is a different feature and not this one badly
        // aimed.
        var shaft = 1.0;
        if !underwater {
            shaft = sun_shaft(frame.cam_pos, rd, frame.far, 1.0, px, py);
        }
        color = sky_color(frame.cam_pos, rd, 0.0, shaft);
    } else {
        let normal_id = u32(key >> 37u) & 7u;
        let ci = u32(key >> 24u) & 0x1FFFu;
        let low = u32(key & 0xFFFFFFlu);
        let v = vec3<u32>((low >> 16u) & VOXEL_MASK, (low >> 8u) & VOXEL_MASK, low & VOXEL_MASK);
        let micro = vec3<u32>((low >> 22u) & 3u, (low >> 14u) & 3u, (low >> 6u) & 3u);

        // Re-derive the exact hit from the stored face rather than storing a depth. The
        // same helper serves `taa`, which reprojects this very point through last frame's
        // matrix; the two have to agree to the bit.
        let t = hit_t(ci, v, micro, normal_id, rd);
        let hit = frame.cam_pos + rd * t;
        let id = block_at(ci, v);
        dist = t;
        // Three materials now, and the *order of the tests* is what keeps the control free:
        // `SPEC_GLASS` is spelled before the id compare so the whole arm -- `shade_glass`, its
        // `trace_world`, its Fresnel -- leaves the module when the override is false, rather
        // than being a branch the driver keeps around for an id it can prove nothing about.
        // That is batch 38's finding applied one flag later: the constant has to reach the
        // use, and an expression with the same value is not the same expression.
        if RESOLVE_LANE == 1u {
            color=shade_glass(ci,v,normal_id,hit,rd,t);
        } else if RESOLVE_LANE == 2u {
            if SPEC_WATER_SEC { color=shade_water_sec(ci,v,normal_id,hit,rd,t,px,py); }
            else { color=shade_water(ci,v,normal_id,hit,rd,t); }
        } else {
            if id == WATER_ID {
                if RESOLVE_LANE == 0u { return; }
                else {
            if SPEC_WATER_SEC { color=shade_water_sec(ci,v,normal_id,hit,rd,t,px,py); }
            else { color=shade_water(ci,v,normal_id,hit,rd,t); }
                }
            } else if SPEC_GLASS && id == GLASS_ID {
                if RESOLVE_LANE == 0u { return; }
                else { color=shade_glass(ci,v,normal_id,hit,rd,t); }
            } else {
                color=shade_hit_cached(ci,v,id,normal_id,hit,rd,t,pix);
            }
        }
        // Aerial perspective, over the part of the path that is actually in air. The target
        // is `sky_base(rd)`, which already carries the one forward-scatter lobe, so haze
        // warms toward the sun without a second halo and a fogged ridge meets the sky above
        // it without a seam. Above water `wet` is zero and this is exactly what batch 5
        // wrote; under it, the haze starts where the ray leaves the water.
        //
        // **The base, deliberately: the deck is not in this mix.** A ridge two kilometres
        // out is hazed by the air in front of it, and that air is lit by the sky behind the
        // ridge -- not by a cloud eight kilometres further on. Mixing toward `sky_color`
        // would print the deck's brightness onto the hillside, brightest exactly where the
        // haze is thickest, which is a light with no source in the frame.
        var wet = 0.0;
        if underwater && SURFACE_DEBUG == 0u {
            wet = water_path(t, rd);
        }
        // The alpha is now needed twice -- once to mix and once to tell `sun_shaft` how much
        // radiance the shafts could possibly move -- so it is named rather than inlined.
        let alpha = 1.0 - transmittance_from(frame.cam_pos.y + wet * rd.y, t - wet, rd);
        var shaft = 1.0;
        if !underwater {
            shaft = sun_shaft(frame.cam_pos, rd, t, alpha, px, py);
        }
        if SURFACE_DEBUG == 0u { color = mix(color, sky_base(rd, shaft), alpha); }
    }
    if underwater && SURFACE_DEBUG == 0u {
        // Batch 54, and **before** the medium rather than after it: this decides what the pixel
        // is looking at, and the medium then absorbs the eye's own leg of whatever that turned
        // out to be. The mirror's leg is absorbed inside.
        color = surface_from_below(color, rd, dist);
        color = water_medium(color, water_path(dist, rd), underwater_body());
    }
    textureStore(out_tex, vec2<i32>(i32(px), i32(py)), vec4<f32>(color, 1.0));
}

// =================== batch 81, roadmap P12's diagnostic pass ===================
//
// Evaluates the two traced water legs for **one pixel per `1<<WATER_SEC_SHIFT` square
// block** (the diagnostic's 2x2 by default) and stores exactly
// the vec4s the leg functions return, so `shade_water_sec` can treat a buffer texel the
// way the classical arm treats a call. The donor is the nearest water pixel of the four,
// the choice that keeps the *content* closest to what a pixel would have traced itself;
// with none, both stores are sentinel writes and no trace runs at all. A shared sample
// may only borrow what is spatially smooth -- hence what is kept per-pixel lives in
// `water_compose`, and what is shared starts and ends at the two function calls below.
@compute @workgroup_size(8, 8, 1)
fn water_sec(@builtin(global_invocation_id) gid: vec3<u32>) {
    let hx = gid.x;
    let hy = gid.y;
    let span = 1u << WATER_SEC_SHIFT;
    let hw = (frame.res.x + span - 1u) >> WATER_SEC_SHIFT;
    let hh = (frame.res.y + span - 1u) >> WATER_SEC_SHIFT;
    if hx >= hw || hy >= hh {
        return;
    }
    var leg_r = vec4<f32>(0.0, 0.0, 0.0, WATER_REFRACT_DIST);
    var leg_m = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var best_t = 3.4e38;
    var has = false;
    var bci = 0u;
    var bv = vec3<u32>(0u, 0u, 0u);
    var bmicro = vec3<u32>(0u, 0u, 0u);
    var bnid = 0u;
    var bpx = 0u;
    var bpy = 0u;
    for (var i = 0u; i < span; i = i + 1u) {
        for (var j = 0u; j < span; j = j + 1u) {
            let pxs = span * hx + i;
            let pys = span * hy + j;
            if pxs >= frame.res.x || pys >= frame.res.y {
                continue;
            }
            let key = atomicLoad(&vis[pys * frame.res.x + pxs]);
            if key == 0lu {
                continue;
            }
            let nids = u32(key >> 37u) & 7u;
            let cis = u32(key >> 24u) & 0x1FFFu;
            let low = u32(key & 0xFFFFFFlu);
            let vs_ = vec3<u32>((low >> 16u) & VOXEL_MASK, (low >> 8u) & VOXEL_MASK, low & VOXEL_MASK);
            let micros = vec3<u32>((low >> 22u) & 3u, (low >> 14u) & 3u, (low >> 6u) & 3u);
            if block_at(cis, vs_) != WATER_ID {
                continue;
            }
            let rd0 = ray_dir(f32(pxs) + 0.5, f32(pys) + 0.5);
            let ts = hit_t(cis, vs_, micros, nids, rd0);
            if ts < best_t {
                best_t = ts;
                bci = cis;
                bv = vs_;
                bmicro = micros;
                bnid = nids;
                bpx = pxs;
                bpy = pys;
                has = true;
            }
        }
    }
    if has {
        let rd_d = ray_dir(f32(bpx) + 0.5, f32(bpy) + 0.5);
        let hit_d = frame.cam_pos + rd_d * best_t;
        let ctx = water_ctx(bci, bv, bnid, hit_d, rd_d, best_t);
        if (frame.flags & FLAG_WATER_REFRACT) != 0u && ctx.refract_detail > 0.0 {
            leg_r = water_refract_leg(hit_d, ctx.n, rd_d, best_t);
        }
        if (frame.flags & FLAG_WATER_REFLECT) != 0u && ctx.reflect_detail > 0.0 {
            leg_m = water_reflect_leg(hit_d, ctx.n, ctx.mirror_t, best_t);
        }
    }
    let hpos = vec2<i32>(i32(hx), i32(hy));
    textureStore(water_refr_tex, hpos, leg_r);
    textureStore(water_refl_tex, hpos, leg_m);
}



// Cached primary schedules are derived from the corresponding control schedules.
// Only local sun visibility is replaced; cloud/envelope/skylight terms stay in place.
// No secondary hit is ever allowed to use this screen-space visibility.
fn shade_hit_legacy_cached(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool, pix: u32) -> vec3<f32> {
        let c = chunks[ci];
        let vs = c.voxel_size;
        let foliage = normal_id >= NORMAL_CROSS_A;
        var n = normal_of(normal_id);
        if foliage && dot(n, rd) > 0.0 {
            n = -n;
        }
        var spec_pre = vec3<f32>(0.0);
        if SPEC_SKY_SPECULAR && do_spec {
            spec_pre = sky_base(reflect(rd, n), 1.0)
                * schlick_ground(clamp(dot(n, -rd), 0.0, 1.0));
        }
        let axis = normal_id >> 1u;
        let box_min = c.origin + vec3<f32>(v) * vs;
        let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));
        let face_word = block_faces[id * 8u + normal_id];
        let layer = face_layer(face_word);
        var uv = face_uv(normal_id, local);
        if SPEC_TEX_VARIATION && (frame.flags & FLAG_TEX_VARIATION) != 0u {
            uv = permute_uv(uv, face_perm(face_word), perm_key(box_min, normal_id));
        }
        var fa: f32;
        var fb: f32;
        var ta: vec3<i32>;
        var tb: vec3<i32>;
        if axis == 0u {
            fa = local.y; fb = local.z;
            ta = vec3<i32>(0, 1, 0); tb = vec3<i32>(0, 0, 1);
        } else if axis == 1u {
            fa = local.x; fb = local.z;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 0, 1);
        } else {
            fa = local.x; fb = local.y;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 1, 0);
        }
        let grazing = max(abs(dot(n, rd)), 0.25);
        let footprint = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs / grazing;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
        let texel = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod);
        var albedo = texel.rgb;
        let tint_mask = select(texel.a, 1.0, foliage);
        if SPEC_TINT && (frame.flags & FLAG_TINT) != 0u && tint_mask > 0.0 {
            let k = tint_mask * frame.tint_strength;
            albedo = albedo * mix(vec3<f32>(1.0), biome_tint(hit.xz), k);
        }
        if SPEC_SHORE_WET && id == SAND_ID {
            let wet = 1.0 - smoothstep(frame.sea_level, frame.sea_level + WET_BAND, hit.y);
            albedo = albedo * mix(1.0, WET_DARK, wet);
        }
        if SPEC_FOLIAGE_RICH
            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))
        {
            let hr = hash_alpha(box_min);
            let hv = hash_alpha(box_min + vec3<f32>(17.31, 7.77, 3.03));
            var rich = vec3<f32>(1.0);
            if hr < 0.16 {
                rich = vec3<f32>(1.30, 1.10, 0.55);
            } else if hr > 0.97 {
                rich = vec3<f32>(1.35, 0.72, 0.95);
            }
            albedo = albedo * rich * mix(0.88, 1.10, hv);
        }
        if SPEC_CANOPY_RELIEF && is_cutout_id(id) {
            let clump = cloud_noise(box_min.xz * 0.21
                + vec2<f32>(box_min.y * 0.313, box_min.y * 0.173));
            let relief = mix(0.72, 1.34, clump * clump);
            let micro = mix(0.92, 1.08, hash_alpha(box_min));
            let toward_sun = clamp(dot(n, frame.sun_dir) * 0.5 + 0.5, 0.0, 1.0);
            albedo = albedo * relief * micro * mix(0.84, 1.0, toward_sun);
        }
        let sky_tint = surface_sky_fill(n);
        var ao: f32 = 1.0;
        var sky_sm: f32 = 0.0;
        var amb: vec3<f32> = vec3<f32>(0.0);
        if foliage {
            let packed = light_at(ci, vec3<i32>(v));
            let sk = light_curve(light_sky(packed)) * frame.daylight;
            sky_sm = sk;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed)), sk, sky_tint);
                } else {
                    let bk = light_curve(light_block_level(packed));
                    amb = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                }
            }
        } else if !smooth_light {
            let air_v1 = vec3<i32>(v) + vec3<i32>(n);
            var packed1: u32;
            if all(air_v1 >= vec3<i32>(0)) && all(air_v1 <= vec3<i32>(63)) {
                packed1 = light_at(ci, air_v1);
            } else {
                packed1 = world_sample(box_min + vec3<f32>(0.5 * vs) + n * vs).x;
            }
            let sk1 = light_curve(light_sky(packed1)) * frame.daylight;
            sky_sm = sk1;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed1)), sk1, sky_tint);
                } else {
                    let bk1 = light_curve(light_block_level(packed1));
                    amb = mix(sky_tint, BLOCK_TINT, bk1 / max(sk1 + bk1, 1e-4)) * max(sk1, bk1);
                }
            }
        } else {
            let air_c = box_min + vec3<f32>(0.5 * vs) + n * vs;
            let step_a = vec3<f32>(ta) * vs;
            let step_b = vec3<f32>(tb) * vs;
            let air_v = vec3<i32>(v) + vec3<i32>(n);
            let span = abs(ta) + abs(tb);
            let local_only = all(air_v - span >= vec3<i32>(0)) && all(air_v + span <= vec3<i32>(63));
            var nb: array<vec2<u32>, 9>;
            if local_only {
                gather_face(ci, air_v, ta, tb, &nb);
            } else {
                for (var i = 0u; i < 9u; i = i + 1u) {
                    nb[i] = world_sample(air_c + step_a * (f32(i % 3u) - 1.0) + step_b * (f32(i / 3u) - 1.0));
                }
            }
            var sky_n: array<f32, 9>;
            var blk_n: array<f32, 9>;
            var open_n: array<f32, 9>;
            var any_blk = 0u;
            for (var i = 0u; i < 9u; i = i + 1u) {
                sky_n[i] = light_curve(light_sky(nb[i].x)) * frame.daylight;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER {
                            any_blk = any_blk | (nb[i].x & 0xFFFu);
                        }
                    } else {
                        blk_n[i] = light_curve(light_block_level(nb[i].x));
                    }
                }
                open_n[i] = 1.0 - f32(nb[i].y);
            }
            var blk4 = vec3<f32>(0.0);
            if SPEC_EMITTER_GATHER && SPEC_LIGHT_RGB && !SPEC_PROBE_AMBIENT && any_blk != 0u {
                blk4 = light_curve3(light_block_rgb(nb[4].x));
            }
            let ao_on = (frame.flags & FLAG_AO) != 0u;
            var amb_c: array<vec3<f32>, 4>;
            var sky_c: array<f32, 4>;
            var ao_c: array<f32, 4>;
            for (var k = 0u; k < 4u; k = k + 1u) {
                let ia = u32(4 + (i32(k & 1u) * 2 - 1));
                let ib = u32(4 + (i32(k >> 1u) * 2 - 1) * 3);
                let ic = ia + ib - 4u;
                let inv = 1.0 / (1.0 + open_n[ia] + open_n[ib] + open_n[ic]);
                let sk = (sky_n[4] + sky_n[ia] * open_n[ia] + sky_n[ib] * open_n[ib] + sky_n[ic] * open_n[ic]) * inv;
                sky_c[k] = sk;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER && any_blk != 0u {
                            let ba = light_curve3(light_block_rgb(nb[ia].x));
                            let bb = light_curve3(light_block_rgb(nb[ib].x));
                            let bc = light_curve3(light_block_rgb(nb[ic].x));
                            let bkv = (blk4 + ba * open_n[ia] + bb * open_n[ib] + bc * open_n[ic]) * inv;
                            amb_c[k] = block_ambient(bkv, sk, sky_tint);
                        } else {
                            amb_c[k] = sky_tint * sk;
                        }
                    } else {
                        let bk = (blk_n[4] + blk_n[ia] * open_n[ia] + blk_n[ib] * open_n[ib] + blk_n[ic] * open_n[ic]) * inv;
                        amb_c[k] = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                    }
                }
                let s1 = 1.0 - open_n[ia];
                let s2 = 1.0 - open_n[ib];
                let occ = select(s1 + s2 + 1.0 - open_n[ic], 3.0, s1 > 0.5 && s2 > 0.5);
                ao_c[k] = select(1.0, 1.0 - occ * AO_STRENGTH, ao_on);
            }
            ao = mix(mix(ao_c[0], ao_c[1], fa), mix(ao_c[2], ao_c[3], fa), fb);
            sky_sm = mix(mix(sky_c[0], sky_c[1], fa), mix(sky_c[2], sky_c[3], fa), fb);
            if !SPEC_PROBE_AMBIENT {
                amb = mix(mix(amb_c[0], amb_c[1], fa), mix(amb_c[2], amb_c[3], fa), fb);
            }
        }
        if SPEC_PROBE_AMBIENT {
            amb = probe_tap(hit);
        } else if SPEC_PROBE_TAP {
            amb = amb * probe_tap(hit);
        }
        var visibility = 1.0;
        var direct = 0.0;
        let ndl = dot(n, frame.sun_dir);
        if ndl > 0.0 && frame.daylight > 0.01 {
            var lit = 1.0;
            if (frame.flags & FLAG_SHADOWS) != 0u {
                lit = textureLoad(primary_sun,vec2<i32>(i32(pix%frame.res.x),i32(pix/frame.res.x)),0).x;
                if SPEC_DISTANT_SHADOWS && (frame.flags & FLAG_DISTANT_SHADOWS) != 0u && lit > 0.0 {
                    lit = lit * terrain_shade_beyond(hit, shadow_dist);
                }
            }
            if lit > 0.0 {
                let cloud_fp = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y)
                    * exp2(frame.mip_bias);
                lit = lit * cloud_shade(hit, cloud_fp);
            }
            visibility = lit;
            direct = lit * ndl * sky_sm;
            if SPEC_CAUSTICS {
                let cdepth = frame.sea_level - hit.y;
                if cdepth > 0.0 && cdepth < CAUSTIC_DEPTH && direct > 0.0 {
                    let sp = hit.xz - frame.sun_dir.xz * (cdepth / max(frame.sun_dir.y, 1e-3));
                    let a = frame.wave_amp;
                    let l0 = frame.wave_scale;
                    let ph = frame.time * frame.wave_speed * WAVE_TAU;
                    let g0 = wave_octave(sp, WAVE_D0, l0, a * 0.42, ph, CAUSTIC_PX,
                                         vec2<f32>(0.0), 0.0, false);
                    let g1 = wave_octave(sp, WAVE_D1, l0 / 2.17, a * 0.27, ph * 1.4731,
                                         CAUSTIC_PX, vec2<f32>(0.0), 0.0, false);
                    let r0 = g0.x * g0.x + g0.y * g0.y;
                    let r1 = g1.x * g1.x + g1.y * g1.y;
                    let sheets = r0 * r1 * CAUSTIC_GAIN;
                    direct = direct * (1.0 + sheets * exp(-cdepth * CAUSTIC_FADE));
                }
            }
        }
        var probe = vec4<f32>(1.0);
        if !SPEC_PROBE_AMBIENT && (SPEC_PROBE_CUBE || SPEC_PROBE_BOUNCE) {
            probe = probe_field(hit + n * PROBE_NORMAL_BIAS, normal_id);
        }
        var shade = 1.0;
        if !SPEC_PROBE_AMBIENT {
            shade = select(face_shade(normal_id), 1.0, SPEC_LIGHTING_REPAIR);
            if SPEC_PROBE_CUBE {
                shade = shade * probe.a;
            }
        }
        let ambient_rgb = amb * 0.50;
        let sun_rgb = vec3<f32>(1.0000, 0.8469, 0.5647) * (direct * select(0.52, 0.78, SPEC_LIGHTING_REPAIR));
        var floor_rgb = vec3<f32>(0.0);
        if !SPEC_PROBE_AMBIENT {
            floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;
            if SPEC_PROBE_BOUNCE {
                floor_rgb = floor_rgb * probe.rgb;
            }
            if SPEC_PROBE_SUN {
                floor_rgb = floor_rgb
                    + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
            }
        }
        if SURFACE_DEBUG == 1u { return n * 0.5 + vec3<f32>(0.5); }
        if SURFACE_DEBUG == 2u { return vec3<f32>(visibility); }
        if SURFACE_DEBUG == 3u { return sun_rgb; }
        if SURFACE_DEBUG == 4u { return ambient_rgb * ao + floor_rgb; }
        let spec_rgb = spec_pre * (sky_sm * SKY_SPEC_GAIN);
        return surface_radiance(albedo, surface_light(ambient_rgb, sun_rgb, ao, floor_rgb, shade), spec_rgb, id);
}

fn shade_hit_compact_cached(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32,
             shadow_dist: f32, smooth_light: bool, do_spec: bool, pix: u32) -> vec3<f32> {
        let c = chunks[ci];
        let vs = c.voxel_size;
        let foliage = normal_id >= NORMAL_CROSS_A;
        var n = normal_of(normal_id);
        if foliage && dot(n, rd) > 0.0 {
            n = -n;
        }
        var spec_pre = vec3<f32>(0.0);
        if SPEC_SKY_SPECULAR && do_spec {
            spec_pre = sky_base(reflect(rd, n), 1.0)
                * schlick_ground(clamp(dot(n, -rd), 0.0, 1.0));
        }
        let grazing = max(abs(dot(n, rd)), 0.25);
        let footprint = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y) * 16.0 / vs / grazing;
        let lod = clamp(log2(max(footprint, 1e-4)) + frame.mip_bias, 0.0, 3.0);
        var light_rgb: vec3<f32>;
        var face_scale: f32;
        var spec_value: vec3<f32>;
        var material_ndl: f32;
        {
        let axis = normal_id >> 1u;
        let box_min = c.origin + vec3<f32>(v) * vs;
        let local = clamp((hit - box_min) / vs, vec3<f32>(0.0), vec3<f32>(1.0));
        var fa: f32;
        var fb: f32;
        var ta: vec3<i32>;
        var tb: vec3<i32>;
        if axis == 0u {
            fa = local.y; fb = local.z;
            ta = vec3<i32>(0, 1, 0); tb = vec3<i32>(0, 0, 1);
        } else if axis == 1u {
            fa = local.x; fb = local.z;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 0, 1);
        } else {
            fa = local.x; fb = local.y;
            ta = vec3<i32>(1, 0, 0); tb = vec3<i32>(0, 1, 0);
        }
        let sky_tint = surface_sky_fill(n);
        var ao: f32 = 1.0;
        var sky_sm: f32 = 0.0;
        var amb: vec3<f32> = vec3<f32>(0.0);
        if foliage {
            let packed = light_at(ci, vec3<i32>(v));
            let sk = light_curve(light_sky(packed)) * frame.daylight;
            sky_sm = sk;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed)), sk, sky_tint);
                } else {
                    let bk = light_curve(light_block_level(packed));
                    amb = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                }
            }
        } else if !smooth_light {
            let air_v1 = vec3<i32>(v) + vec3<i32>(n);
            var packed1: u32;
            if all(air_v1 >= vec3<i32>(0)) && all(air_v1 <= vec3<i32>(63)) {
                packed1 = light_at(ci, air_v1);
            } else {
                packed1 = world_sample(box_min + vec3<f32>(0.5 * vs) + n * vs).x;
            }
            let sk1 = light_curve(light_sky(packed1)) * frame.daylight;
            sky_sm = sk1;
            if !SPEC_PROBE_AMBIENT {
                if SPEC_LIGHT_RGB {
                    amb = block_ambient(light_curve3(light_block_rgb(packed1)), sk1, sky_tint);
                } else {
                    let bk1 = light_curve(light_block_level(packed1));
                    amb = mix(sky_tint, BLOCK_TINT, bk1 / max(sk1 + bk1, 1e-4)) * max(sk1, bk1);
                }
            }
        } else {
            let air_c = box_min + vec3<f32>(0.5 * vs) + n * vs;
            let step_a = vec3<f32>(ta) * vs;
            let step_b = vec3<f32>(tb) * vs;
            let air_v = vec3<i32>(v) + vec3<i32>(n);
            let span = abs(ta) + abs(tb);
            let local_only = all(air_v - span >= vec3<i32>(0)) && all(air_v + span <= vec3<i32>(63));
            var nb: array<vec2<u32>, 9>;
            if local_only {
                gather_face(ci, air_v, ta, tb, &nb);
            } else {
                for (var i = 0u; i < 9u; i = i + 1u) {
                    nb[i] = world_sample(air_c + step_a * (f32(i % 3u) - 1.0) + step_b * (f32(i / 3u) - 1.0));
                }
            }
            var sky_n: array<f32, 9>;
            var blk_n: array<f32, 9>;
            var open_n: array<f32, 9>;
            var any_blk = 0u;
            for (var i = 0u; i < 9u; i = i + 1u) {
                sky_n[i] = light_curve(light_sky(nb[i].x)) * frame.daylight;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER {
                            any_blk = any_blk | (nb[i].x & 0xFFFu);
                        }
                    } else {
                        blk_n[i] = light_curve(light_block_level(nb[i].x));
                    }
                }
                open_n[i] = 1.0 - f32(nb[i].y);
            }
            var blk4 = vec3<f32>(0.0);
            if SPEC_EMITTER_GATHER && SPEC_LIGHT_RGB && !SPEC_PROBE_AMBIENT && any_blk != 0u {
                blk4 = light_curve3(light_block_rgb(nb[4].x));
            }
            let ao_on = (frame.flags & FLAG_AO) != 0u;
            var amb_c: array<vec3<f32>, 4>;
            var sky_c: array<f32, 4>;
            var ao_c: array<f32, 4>;
            for (var k = 0u; k < 4u; k = k + 1u) {
                let ia = u32(4 + (i32(k & 1u) * 2 - 1));
                let ib = u32(4 + (i32(k >> 1u) * 2 - 1) * 3);
                let ic = ia + ib - 4u;
                let inv = 1.0 / (1.0 + open_n[ia] + open_n[ib] + open_n[ic]);
                let sk = (sky_n[4] + sky_n[ia] * open_n[ia] + sky_n[ib] * open_n[ib] + sky_n[ic] * open_n[ic]) * inv;
                sky_c[k] = sk;
                if !SPEC_PROBE_AMBIENT {
                    if SPEC_LIGHT_RGB {
                        if SPEC_EMITTER_GATHER && any_blk != 0u {
                            let ba = light_curve3(light_block_rgb(nb[ia].x));
                            let bb = light_curve3(light_block_rgb(nb[ib].x));
                            let bc = light_curve3(light_block_rgb(nb[ic].x));
                            let bkv = (blk4 + ba * open_n[ia] + bb * open_n[ib] + bc * open_n[ic]) * inv;
                            amb_c[k] = block_ambient(bkv, sk, sky_tint);
                        } else {
                            amb_c[k] = sky_tint * sk;
                        }
                    } else {
                        let bk = (blk_n[4] + blk_n[ia] * open_n[ia] + blk_n[ib] * open_n[ib] + blk_n[ic] * open_n[ic]) * inv;
                        amb_c[k] = mix(sky_tint, BLOCK_TINT, bk / max(sk + bk, 1e-4)) * max(sk, bk);
                    }
                }
                let s1 = 1.0 - open_n[ia];
                let s2 = 1.0 - open_n[ib];
                let occ = select(s1 + s2 + 1.0 - open_n[ic], 3.0, s1 > 0.5 && s2 > 0.5);
                ao_c[k] = select(1.0, 1.0 - occ * AO_STRENGTH, ao_on);
            }
            ao = mix(mix(ao_c[0], ao_c[1], fa), mix(ao_c[2], ao_c[3], fa), fb);
            sky_sm = mix(mix(sky_c[0], sky_c[1], fa), mix(sky_c[2], sky_c[3], fa), fb);
            if !SPEC_PROBE_AMBIENT {
                amb = mix(mix(amb_c[0], amb_c[1], fa), mix(amb_c[2], amb_c[3], fa), fb);
            }
        }
        if SPEC_PROBE_AMBIENT {
            amb = probe_tap(hit);
        } else if SPEC_PROBE_TAP {
            amb = amb * probe_tap(hit);
        }
        var visibility = 1.0;
        var direct = 0.0;
        let ndl = dot(n, frame.sun_dir);
        if ndl > 0.0 && frame.daylight > 0.01 {
            var lit = 1.0;
            if (frame.flags & FLAG_SHADOWS) != 0u {
                lit = textureLoad(primary_sun,vec2<i32>(i32(pix%frame.res.x),i32(pix/frame.res.x)),0).x;
                if SPEC_DISTANT_SHADOWS && (frame.flags & FLAG_DISTANT_SHADOWS) != 0u && lit > 0.0 {
                    lit = lit * terrain_shade_beyond(hit, shadow_dist);
                }
            }
            if lit > 0.0 {
                let cloud_fp = max(dist, 1e-3) * 2.0 * frame.tan_half_fov / f32(frame.res.y)
                    * exp2(frame.mip_bias);
                lit = lit * cloud_shade(hit, cloud_fp);
            }
            visibility = lit;
            direct = lit * ndl * sky_sm;
            if SPEC_CAUSTICS {
                let cdepth = frame.sea_level - hit.y;
                if cdepth > 0.0 && cdepth < CAUSTIC_DEPTH && direct > 0.0 {
                    let sp = hit.xz - frame.sun_dir.xz * (cdepth / max(frame.sun_dir.y, 1e-3));
                    let a = frame.wave_amp;
                    let l0 = frame.wave_scale;
                    let ph = frame.time * frame.wave_speed * WAVE_TAU;
                    let g0 = wave_octave(sp, WAVE_D0, l0, a * 0.42, ph, CAUSTIC_PX,
                                         vec2<f32>(0.0), 0.0, false);
                    let g1 = wave_octave(sp, WAVE_D1, l0 / 2.17, a * 0.27, ph * 1.4731,
                                         CAUSTIC_PX, vec2<f32>(0.0), 0.0, false);
                    let r0 = g0.x * g0.x + g0.y * g0.y;
                    let r1 = g1.x * g1.x + g1.y * g1.y;
                    let sheets = r0 * r1 * CAUSTIC_GAIN;
                    direct = direct * (1.0 + sheets * exp(-cdepth * CAUSTIC_FADE));
                }
            }
        }
        var probe = vec4<f32>(1.0);
        if !SPEC_PROBE_AMBIENT && (SPEC_PROBE_CUBE || SPEC_PROBE_BOUNCE) {
            probe = probe_field(hit + n * PROBE_NORMAL_BIAS, normal_id);
        }
        var shade = 1.0;
        if !SPEC_PROBE_AMBIENT {
            shade = select(face_shade(normal_id), 1.0, SPEC_LIGHTING_REPAIR);
            if SPEC_PROBE_CUBE {
                shade = shade * probe.a;
            }
        }
        let ambient_rgb = amb * 0.50;
        let sun_rgb = vec3<f32>(1.0000, 0.8469, 0.5647) * (direct * select(0.52, 0.78, SPEC_LIGHTING_REPAIR));
        var floor_rgb = vec3<f32>(0.0);
        if !SPEC_PROBE_AMBIENT {
            floor_rgb = vec3<f32>(0.85, 0.90, 1.00) * frame.ambient;
            if SPEC_PROBE_BOUNCE {
                floor_rgb = floor_rgb * probe.rgb;
            }
            if SPEC_PROBE_SUN {
                floor_rgb = floor_rgb
                    + vec3<f32>(0.85, 0.90, 1.00) * probe.rgb * (probe_sun_gain() * sky_sm);
            }
        }
        if SURFACE_DEBUG == 1u { return n * 0.5 + vec3<f32>(0.5); }
        if SURFACE_DEBUG == 2u { return vec3<f32>(visibility); }
        if SURFACE_DEBUG == 3u { return sun_rgb; }
        if SURFACE_DEBUG == 4u { return ambient_rgb * ao + floor_rgb; }
        let spec_rgb = spec_pre * (sky_sm * SKY_SPEC_GAIN);
        light_rgb = surface_light(ambient_rgb, sun_rgb, ao, floor_rgb, shade);
        face_scale = 1.0;
        spec_value = spec_rgb;
        material_ndl = ndl;
        }
        let material_chunk = chunks[ci];
        let box_min = material_chunk.origin + vec3<f32>(v) * material_chunk.voxel_size;
        let local = clamp((hit - box_min) / material_chunk.voxel_size, vec3<f32>(0.0), vec3<f32>(1.0));
        let face_word = block_faces[id * 8u + normal_id];
        let layer = face_layer(face_word);
        var uv = face_uv(normal_id, local);
        if SPEC_TEX_VARIATION && (frame.flags & FLAG_TEX_VARIATION) != 0u {
            uv = permute_uv(uv, face_perm(face_word), perm_key(box_min, normal_id));
        }
        let texel = textureSampleLevel(atlas, atlas_samp, uv, i32(layer), lod);
        var albedo = texel.rgb;
        let tint_mask = select(texel.a, 1.0, foliage);
        if SPEC_TINT && (frame.flags & FLAG_TINT) != 0u && tint_mask > 0.0 {
            let k = tint_mask * frame.tint_strength;
            albedo = albedo * mix(vec3<f32>(1.0), biome_tint(hit.xz), k);
        }
        if SPEC_SHORE_WET && id == SAND_ID {
            let wet = 1.0 - smoothstep(frame.sea_level, frame.sea_level + WET_BAND, hit.y);
            albedo = albedo * mix(1.0, WET_DARK, wet);
        }
        if SPEC_FOLIAGE_RICH
            && (foliage || ((id == GRASS_ID || id == MEADOW_ID) && normal_id == NORMAL_PY))
        {
            let hr = hash_alpha(box_min);
            let hv = hash_alpha(box_min + vec3<f32>(17.31, 7.77, 3.03));
            var rich = vec3<f32>(1.0);
            if hr < 0.16 {
                rich = vec3<f32>(1.30, 1.10, 0.55);
            } else if hr > 0.97 {
                rich = vec3<f32>(1.35, 0.72, 0.95);
            }
            albedo = albedo * rich * mix(0.88, 1.10, hv);
        }
        if SPEC_CANOPY_RELIEF && is_cutout_id(id) {
            let clump = cloud_noise(box_min.xz * 0.21
                + vec2<f32>(box_min.y * 0.313, box_min.y * 0.173));
            let relief = mix(0.72, 1.34, clump * clump);
            let micro = mix(0.92, 1.08, hash_alpha(box_min));
            let toward_sun = clamp(material_ndl * 0.5 + 0.5, 0.0, 1.0);
            albedo = albedo * relief * micro * mix(0.84, 1.0, toward_sun);
        }
        return surface_radiance(albedo, light_rgb * face_scale, spec_value, id);
}
fn shade_hit_cached(ci: u32, v: vec3<u32>, id: u32, normal_id: u32, hit: vec3<f32>, rd: vec3<f32>, dist: f32, pix: u32) -> vec3<f32> {
    if SPEC_COMPACT_SHADE_HIT {
        return shade_hit_compact_cached(ci,v,id,normal_id,hit,rd,dist,frame.shadow_dist,true,true,pix);
    }
    return shade_hit_legacy_cached(ci,v,id,normal_id,hit,rd,dist,frame.shadow_dist,true,true,pix);
}

// Every pixel is initialized, including sky, heatmap and transparent surfaces.
// Called only after final visibility (including recovery marching) is complete.

