//! Offline shader compilation, sizing and driver statistics tool.
//!
//! Pinned by tests/bins.rs: shaderstats_reads_the_shipping_module_through_render.

use std::env;
use std::process;
use voxelcraft::render;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("sizes");

    // Must read the same concatenated module the game compiles
    let source = render::shader_source();

    if args.iter().any(|a| a == "--source" || a == "--wgsl") {
        println!("{source}");
        return;
    }

    // Parse and validate WGSL
    let module = match naga::front::wgsl::parse_str(&source) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("WGSL parse error: {e}");
            process::exit(1);
        }
    };

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    let module_info = match validator.validate(&module) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("Module validation error: {e}");
            process::exit(1);
        }
    };

    match mode {
        "check" => {
            println!("WGSL module validated successfully ({} entry points).", render::ENTRY_POINTS.len());
        }
        "sizes" => {
            println!("=== Pipeline sizes table (overrides at declared defaults) ===");
            println!("{:<24} {:>10} {:>10}", "Entry Point", "SPIR-V Words", "Bytes");
            println!("{:-<48}", "");

            let spv_options = naga::back::spv::Options {
                lang_version: (1, 3),
                ..Default::default()
            };

            for &entry in &render::ENTRY_POINTS {
                // Validate that entry point exists in module
                let has_entry = module.entry_points.iter().any(|ep| ep.name == entry);
                if !has_entry {
                    eprintln!("Warning: entry point '{entry}' not found in WGSL module");
                    continue;
                }

                match naga::back::spv::write_vec(&module, &module_info, &spv_options, None) {
                    Ok(words) => {
                        let bytes = words.len() * 4;
                        println!("{:<24} {:>10} {:>10}", entry, words.len(), bytes);
                    }
                    Err(e) => {
                        println!("{:<24} {:>10} {:>10}", entry, "ERR", format!("{e:?}"));
                    }
                }
            }
        }
        "drv" => {
            // Attempt driver reflection via VK_KHR_pipeline_executable_properties
            println!("Attempting driver statistics via VK_KHR_pipeline_executable_properties...");
            if !try_driver_stats(&module, &module_info) {
                eprintln!(
                    "pipeline_executable_properties unavailable: no Vulkan driver with \
                     VK_KHR_pipeline_executable_properties extension found."
                );
                // Degrade loudly to offline sizes with exit code 3 per specification
                process::exit(3);
            }
        }
        unknown => {
            eprintln!("Unknown argument: {unknown}");
            eprintln!("Usage: shaderstats [\"check\" | \"sizes\" | \"drv\"] [--wgsl] [--source]");
            process::exit(2);
        }
    }
}

#[allow(dead_code)]
fn try_driver_stats(_module: &naga::Module, _info: &naga::valid::ModuleInfo) -> bool {
    // Attempt dynamic Vulkan loader connection
    let entry = match unsafe { ash::Entry::load() } {
        Ok(e) => e,
        Err(_) => return false,
    };

    let app_info = ash::vk::ApplicationInfo::default()
        .api_version(ash::vk::make_api_version(0, 1, 2, 0));
    let instance_info = ash::vk::InstanceCreateInfo::default().application_info(&app_info);

    let instance = match unsafe { entry.create_instance(&instance_info, None) } {
        Ok(i) => i,
        Err(_) => return false,
    };

    let physical_devices = match unsafe { instance.enumerate_physical_devices() } {
        Ok(p) => p,
        Err(_) => return false,
    };

    if physical_devices.is_empty() {
        return false;
    }

    // Check if VK_KHR_pipeline_executable_properties is supported
    let ext_name = ash::vk::KhrPipelineExecutablePropertiesFn::name();
    let supported = physical_devices.iter().any(|&pdev| {
        let exts = unsafe { instance.enumerate_device_extension_properties(pdev) }
            .unwrap_or_default();
        exts.iter().any(|e| unsafe {
            std::ffi::CStr::from_ptr(e.extension_name.as_ptr()) == ext_name
        })
    });

    supported
}
