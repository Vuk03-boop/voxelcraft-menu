//! GPU capability probe: prints adapter identity and queries features for the
//! ray-tracing, subgroup and cooperative-matrix decision rungs.
//!
//! Pinned by tests/bins.rs: probe_names_the_features_the_loop_decides_on.

fn main() {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::all();
    let instance = wgpu::Instance::new(desc);

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .or_else(|| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    })
    .expect("failed to find an adapter");

    let info = adapter.get_info();
    println!("adapter: {} ({:?}, {:?})", info.name, info.device_type, info.backend);

    let features = adapter.features();
    let limits = adapter.limits();

    let feat_dump = format!("{:#?}", features);
    let lim_dump = format!("{:#?}", limits);

    println!("features dump:\n{feat_dump}");
    println!("limits dump:\n{lim_dump}");

    // Features the engineering loop decides on:
    let named_features = [
        "EXPERIMENTAL_RAY_QUERY",
        "RAY_HIT_VERTEX_RETURN",
        "RAY_TRACING_PIPELINES",
        "COOPERATIVE_MATRIX",
    ];

    println!("\ndecision features:");
    for feat in named_features {
        let present = feat_dump.contains(feat);
        println!("  {feat}: {present}");
    }
}
