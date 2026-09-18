//! Batch 102 pins for the binary trilogy (`src/bin/`, brand new this batch): the
//! engineering loop speaks in the tools' printed lines (`probe`, `harness bench`,
//! `shaderstats`), and the names above are how a round finds the line it is looking
//! for. Checked as literal substrings like the `docs.rs` class: refactor freely;
//! rename and the pin tells you which docstring, run command or handoff to change
//! with it.

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
    // The nine names `docs/harness.md` documents and the usage errors print.
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
    // `--mip0` maps onto the engine's actual flag; the docstring must keep naming
    // the destination so the round knows what is really being set.
    assert!(
        h.contains("--mip-bias"),
        "harness must document that --mip0 sets the engine's --mip-bias 0"
    );
    // The two relay contracts a round reads off stdout: march's integer counts and
    // the reference's noise floor, both defined by the engine and required by the
    // driver.
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
    // And it must print the adapter identity first: G1's law is that a bank of
    // numbers without its machine is an anecdote.
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
    // The sizes table must label itself "overrides at declared defaults", so a
    // round never mistakes an unspecialized size for a shipping one (A5's rule).
    assert!(
        s.contains("declared defaults"),
        "shaderstats must label its sizes table as unspecialized"
    );
}

