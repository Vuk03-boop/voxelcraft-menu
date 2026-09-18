use voxelcraft::config::{parse_from, Config};
use voxelcraft::render::{self, FLAG_HI_SNELL_BEND};

fn cfg_of(args: &[&str]) -> Config {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (cfg, _, unknown) = parse_from(&owned);
    assert!(unknown.is_empty(), "unexpected unknown args: {unknown:?}");
    cfg
}

#[test]
fn a2c_flag_defaults_off_and_parses() {
    assert!(!Config::default().snell_bend);
    assert!(cfg_of(&["--snell-bend"]).snell_bend);
}

#[test]
fn a2c_bit_allocation() {
    assert_eq!(FLAG_HI_SNELL_BEND, 256);
}

#[test]
fn a2c_override_default_is_dark() {
    let src = render::shader_source();
    assert!(src.contains("override SPEC_SNELL_BEND: bool = false;"));
}

#[test]
fn a2c_resolve_gains_nothing() {
    let resolve = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/render/shaders/resolve.wgsl"
    ))
    .expect("resolve.wgsl");
    assert!(
        !resolve.contains("SPEC_SNELL_BEND"),
        "resolve read the bend's override -- see the pin's comment"
    );
}

#[test]
fn a2c_one_surface_epsilon() {
    let common = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/render/shaders/common.wgsl"
    ))
    .expect("common.wgsl");
    assert!(common.contains("const SURFACE_EPS: f32 = 0.02;"));
    let mut defs = 0;
    for f in ["resolve.wgsl", "march.wgsl", "common.wgsl"] {
        let src = std::fs::read_to_string(format!(
            "{}/src/render/shaders/{f}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect(f);
        defs += src.matches("const SURFACE_EPS").count();
    }
    assert_eq!(defs, 1, "SURFACE_EPS is defined exactly once, in common.wgsl");
}
