use std::path::Path;

fn source(rel: &str) -> Option<String> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    if !p.exists() {
        return None;
    }
    Some(std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{rel}: {e}")))
}

#[test]
fn harness_subcommands_are_the_documented_vocabulary() {
    let Some(h) = source("src/bin/harness.rs") else { return; };

    for sub in [
        "\"list\"",
        "\"capture\"",
        "\"validate\"",
        "\"implies\"",
        "\"bitexact\"",
        "\"bench\"",
        "\"march\"",
        "\"reference\"",
        "\"compare\"",
    ] {
        assert!(h.contains(sub), "harness lost subcommand {sub}");
    }
}

#[test]
fn harness_flags_are_the_documented_vocabulary() {
    let Some(h) = source("src/bin/harness.rs") else { return; };
    for flag in [
        "--vantage",
        "--only",
        "--rounds",
        "--out",
        "--rect",
        "--crop",
        "--tiles",
        "--profile",
        "--mip0",
        "--taa",
        "--samples",
        "--exe-a",
        "--exe-b",
        "--extra",
        "--extra-b",
    ] {
        assert!(h.contains(flag), "harness lost flag {flag}");
    }

    assert!(
        h.contains("--mip-bias"),
        "harness must document that --mip0 sets the engine's --mip-bias 0"
    );

    assert!(
        h.contains("march-stats:"),
        "harness must relay the engine's march-stats lines"
    );
    assert!(
        h.contains("noise floor"),
        "harness must refuse a reference run with no noise-floor line"
    );
}

#[test]
fn probe_names_the_features_the_loop_decides_on() {
    let Some(p) = source("src/bin/probe.rs") else { return; };
    for feature in [
        "EXPERIMENTAL_RAY_QUERY",
        "RAY_HIT_VERTEX_RETURN",
        "RAY_TRACING_PIPELINES",
        "COOPERATIVE_MATRIX",
    ] {
        assert!(p.contains(feature), "probe lost named feature {feature}");
    }

    assert!(p.contains("adapter:"), "probe lost the adapter identity line");
}

#[test]
fn shaderstats_reads_the_shipping_module_through_render() {
    let Some(s) = source("src/bin/shaderstats.rs") else { return; };
    assert!(
        s.contains("render::shader_source()"),
        "shaderstats must read the same concatenated module the game compiles"
    );
    assert!(
        s.contains("render::ENTRY_POINTS"),
        "shaderstats must walk the passes via render::ENTRY_POINTS"
    );
    for arg in ["\"check\"", "\"sizes\"", "\"drv\"", "--wgsl", "--source"] {
        assert!(s.contains(arg), "shaderstats lost argument {arg}");
    }
    assert!(
        s.contains("pipeline_executable_properties"),
        "shaderstats lost the VK_KHR_pipeline_executable_properties driver half"
    );

    assert!(
        s.contains("declared defaults"),
        "shaderstats must label its sizes table as unspecialized"
    );
}

#[test]
fn the_retired_water_mottle_flag_owns_up_in_help() {
    let Some(cfg) = source("src/config.rs") else {
        panic!("src/config.rs is the one parser and --help the one vocabulary");
    };
    assert!(
        cfg.contains("\"--water-mottle\""),
        "the retired flag must keep PARSING (harmlessly) so old launch lines do not break"
    );
    assert!(
        cfg.contains("cfg.water_mottle=0.0"),
        "a retired flag must be force-zeroed on parse, not threaded into a dead uniform"
    );
    assert!(
        cfg.contains("retired: parsed, warned, force-zeroed"),
        "the --help row must OWN the retirement -- the sticky version of this bug was a row \
         that promised to reproduce a build the parser was silently zeroing"
    );
    assert!(
        !cfg.contains("0.2 = pre-batch-26"),
        "the help row must never again claim bit-identity with a build that no longer exists"
    );
}

#[test]
fn shadow_dist_is_a_flag_not_a_hardcode() {
    let cfg = source("src/config.rs").expect("src/config.rs is the one parser");
    let r = source("src/render/mod.rs").expect("the frame upload lives here");
    assert!(
        cfg.contains("--shadow-dist"),
        "the shadow reach must be a CLI flag -- the measurement plan needs it"
    );
    assert!(
        cfg.contains("shadow_dist: f32"),
        "Config must carry the field the flag feeds"
    );
    assert!(
        cfg.contains("shadow_dist: 220.0"),
        "and its default must stay the historical 220.0, or every cached baseline shifts"
    );
    assert!(
        !r.contains("shadow_dist: 220.0"),
        "the hardcode crept back -- the whole point of the flag is that this number moves"
    );
    assert!(
        r.contains("shadow_dist: p.shadow_dist,"),
        "the frame upload must read the flag through FrameParams"
    );
}

#[test]
fn bench_frames_honors_the_demo_builders() {
    let h = source("src/headless.rs").expect("headless benches live here");
    let at = h
        .find("fn bench_frames(")
        .expect("bench_frames is the timing harness");
    let window = &h[at..(at + 8000).min(h.len())];
    assert!(
        window.contains("build_glass_structure"),
        "the bench must see the same demo worlds the captures see -- glass perf arms \
         otherwise time an empty scene and call it cheap"
    );
    assert!(
        window.contains("rebuild_dirty_now"),
        "demo edits must be rebuilt into the coarse stand-ins before the stopwatch starts"
    );
}

#[test]
fn world_epoch_reaches_the_gpu_frame() {
    let r = source("src/render/mod.rs").expect("the frame upload lives here");
    let a = source("src/app.rs").expect("the interactive path builds FrameParams here");
    let h = source("src/headless.rs").expect("the harness path builds FrameParams here");
    let wgsl = source("src/render/shaders/common.wgsl").expect("the shader Frame twin lives here");
    assert!(
        r.contains("pub world_epoch: u32,"),
        "FrameParams must carry the epoch -- experiments that reuse history key off it"
    );
    assert!(
        r.contains("world_epoch: p.world_epoch,"),
        "the upload site must read the epoch through FrameParams"
    );
    assert!(
        r.contains("offset_of!(GpuFrame, world_epoch) == 352"),
        "GpuFrame layout pins must track the appended tail field"
    );
    assert!(
        r.contains("size_of::<GpuFrame>() == 368"),
        "352 + u32 + 12 pad = 368 and 16-aligned -- a shifted matrix is a corrupted frame"
    );
    assert!(
        a.contains("world_epoch: self.world.version as u32,"),
        "the interactive path feeds it from World.version"
    );
    assert!(
        h.matches("world_epoch: world.version as u32,").count() == 2,
        "both headless builders (capture + bench) must feed it too"
    );
    assert!(
        wgsl.contains("world_epoch: u32,"),
        "the shader-side Frame struct must mirror the payload or offsets lie"
    );
}

#[test]
fn exp_shadow_repro_skips_only_when_the_signature_matches() {
    let cfg = source("src/config.rs").expect("the one parser");
    let r = source("src/render/mod.rs").expect("the dispatch lives here");
    let a = source("src/app.rs").expect("interactive builder");
    let h = source("src/headless.rs").expect("headless builders");
    assert!(
        cfg.contains("--exp-shadow-repro") && cfg.contains("exp_shadow_repro: bool"),
        "the experiment must be a real CLI flag with a Config field, default off"
    );
    assert!(
        cfg.contains("exp_shadow_repro: false"),
        "experiments are off by default -- main behavior and every EXACT pin stay untouched"
    );
    assert!(
        r.contains("pub exp_shadow_repro: bool,"),
        "FrameParams must carry the experiment through"
    );
    assert!(
        r.contains("let reuse_shadow = p.exp_shadow_repro && self.shadow_sig == Some(shadow_sig);"),
        "the skip must be AND-gated on the flag -- silent reuse on a bare signature is a cache bug"
    );
    assert!(
        r.contains("fn steady_signature("),
        "the signature covers camera+lens+sun+reach+spec+size+epoch so one bit moving re-traces"
    );
    assert!(
        r.contains("self.shadow_sig = Some(shadow_sig);"),
        "running the trio must seal the next frame\u{2019}s comparison"
    );
    assert!(
        r.contains("self.shadow_sig = None;") && a.contains("exp_shadow_repro: self.cfg.exp_shadow_repro,")
            && h.matches("exp_shadow_repro: cfg.exp_shadow_repro,").count() == 2,
        "target recreation forgets the signature; all three builders thread the flag"
    );
}

#[test]
fn exp_temporal_hiz_reuses_under_the_same_steadiness_contract() {
    let cfg = source("src/config.rs").expect("the one parser");
    let r = source("src/render/mod.rs").expect("the dispatch lives here");
    assert!(
        cfg.contains("--exp-temporal-hiz") && cfg.contains("exp_temporal_hiz: false"),
        "exp flag with the default-off contract"
    );
    assert!(
        r.contains("pub exp_temporal_hiz: bool,"),
        "FrameParams must carry it"
    );
    assert!(
        r.contains("let reuse_hiz = p.exp_temporal_hiz && self.hiz_sig == Some(hiz_sig);"),
        "the skip must be AND-gated on the flag and sealed by the signature"
    );
    assert!(
        r.contains("self.hiz_sig = Some(hiz_sig);"),
        "running the build seals the next frame\u{2019}s comparison"
    );
    assert!(
        r.contains("self.hiz_sig = None;"),
        "size-change recreates must forget the signature -- tiles derive from the surface size"
    );
    assert!(
        r.contains("fn shadow_signature") == false,
        "the signature was renamed: it describes frame steadiness, used by shadow AND hiz reuse"
    );
}

#[test]
fn exp_halfres_glass_runs_legs_once_per_quadrant() {
    let r = source("src/render/mod.rs").expect("dispatch + wiring");
    let wgsl = source("src/render/shaders/resolve.wgsl").expect("the lane shader");
    let common = source("src/render/shaders/common.wgsl").expect("overrides");
    let cfg = source("src/config.rs").expect("flags");
    assert!(
        cfg.contains("--exp-halfres-glass") && cfg.contains("exp_halfres_glass: false"),
        "exp contract: real flag, default off"
    );
    assert!(
        common.contains("override SPEC_GLASS_QUAD: bool = false;"),
        "the override the wiring pair feeds"
    );
    assert!(
        r.contains("FLAG_HI_GLASS_QUAD: u32 = 536870912;"),
        "next free spec_hi bit after SUN_WIDE (1<<28; LIGHT_RGB\u{2019}s 268435456 is the BASE mask)"
    );
    assert!(
        r.contains("\"SPEC_GLASS_QUAD\","),
        "the constants pair must wire it (spec_hi guard agreement)"
    );
    assert!(
        r.contains("label == \"glass resolve\" && spec_key.1 & FLAG_HI_GLASS_QUAD != 0"),
        "only the glass lane dispatches the halved grid"
    );
    assert!(
        wgsl.contains("fn resolve_pixel") && wgsl.contains("if SPEC_GLASS_QUAD && RESOLVE_LANE == 1u"),
        "quadrant wrapper around the parametrized pixel body, lane-1-only and spec-gated"
    );
    assert!(
        wgsl.matches("shade_glass_shared").count() == 3,
        "one wrapper def + two call sites, no third leg leaking around the cache"
    );
}
