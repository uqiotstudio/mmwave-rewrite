use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::fs;

fn main() {
    // Update ld_library_path
    let ld_library_path = env::var("LD_LIBRARY_PATH").unwrap_or_default();
    let new_ld_library_path = format!("{}:{}/cpp/build", ld_library_path, env::current_dir().unwrap().display());
    println!("cargo:rustc-env=LD_LIBRARY_PATH={}", new_ld_library_path);

    
    // Define the path to your C++ source files and build directory
    let cpp_source_dir = "cpp";
    let build_dir = "cpp/build";

    // Check if the C++ files have changed
    let cpp_files_changed = fs::read_dir(cpp_source_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            let path = entry.path();
            let metadata = fs::metadata(&path).unwrap();
            let last_modified = metadata.modified().unwrap();
            println!("cargo:rerun-if-changed={}", path.display());
            last_modified > metadata.accessed().unwrap()
        });


    // Run 'make' in the build directory if C++ files have changed
    if cpp_files_changed {
        println!("Running 'make' in the C++ build directory...");
        let output = Command::new("make")
            .current_dir(build_dir)
            .output()
            .expect("Failed to execute 'make'");

        if !output.status.success() {
            panic!("Failed to build C++ library: {}", String::from_utf8_lossy(&output.stderr));
        }
    }


    // Link the shared library
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let zed_lib_path = PathBuf::from(manifest_dir.clone()).join(build_dir);

    // Debugging information
    println!("Manifest directory: {}", manifest_dir);
    println!("Shared library directory: {}", zed_lib_path.display());
    println!("cargo:rustc-link-search=native={}", zed_lib_path.display());
    println!("cargo:rustc-link-lib=dylib=zed_interface_lib");

    // Check if the shared library file exists
    let shared_lib_path = zed_lib_path.join("libzed_interface_lib.so");
    if shared_lib_path.exists() {
        println!("Confirmed: Shared library file exists at {}", shared_lib_path.display());
    } else {
        println!("Warning: Shared library file not found at {}", shared_lib_path.display());
    }
}
