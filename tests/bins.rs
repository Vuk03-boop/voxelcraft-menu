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
