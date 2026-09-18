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
