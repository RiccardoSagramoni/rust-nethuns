use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};


cfg_if::cfg_if! {
    if #[cfg(feature="netmap")] {
        const WRAPPER_PATH: &str = "bindgen_wrappers/netmap.h";
    } else {
        std::compile_error!("The support for the specified I/O framework is not available yet. Check the documentation for more information.");
    }
}
const LINK_NAME: &str = "wrapper.h";


fn main() {
    // Check if have been enabled more than one feature flag
    // for the underlying I/O framework
    assert_io_framework_mutual_exclusivity();
    
    // Create symlink for wrapper.h
    let _ = fs::remove_file(LINK_NAME);
    symlink(WRAPPER_PATH, LINK_NAME)
        .expect("Failed to create wrapper.h symlink");
    
    // Tell cargo to look for shared libraries in the specified directory.
    println!("cargo:rustc-link-search=/usr/local/lib/");
    
    // Tell cargo to tell rustc to link the system shared libraries.
    println!("cargo:rustc-link-lib=netmap");
    
    // Tell cargo to invalidate the built crate whenever the wrapper changes.
    println!("cargo:rerun-if-changed={WRAPPER_PATH}");
    
    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(LINK_NAME)
        // Automatically derive the Default trait whenever possible
        .derive_default(true)
        // Genereate wrappers for static functions
        .wrap_static_fns(true)
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");
    
    compile_extern_source_code();
    
    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
    
    // Destroy the symlink
    fs::remove_file(LINK_NAME).expect("Failed to remove wrapper.h symlink");
}


/// Check the feature flags for the underlying I/O frameworks.
///
/// # Panics
/// If none or more than one feature flag have been enabled for the underlying I/O framework.
fn assert_io_framework_mutual_exclusivity() {
    let mut found: u8 = 0;
    
    cfg_if::cfg_if! {
        if #[cfg(feature="netmap")] {
            found += 1;
        }
    };
    cfg_if::cfg_if! {
        if #[cfg(feature="libpcap")] {
            found += 1;
        }
    };
    cfg_if::cfg_if! {
        if #[cfg(feature="xdp")] {
            found += 1;
        }
    };
    cfg_if::cfg_if! {
        if #[cfg(feature="tpacket_v3")] {
            found += 1;
        }
    };
    
    if found == 0 {
        panic!("Error: no IO framework found");
    }
    if found > 1 {
        panic!("Error: more than one IO framework found");
    }
}


/// Compile the C source code generated by the `wrap_static_fns` option
/// for the bindgen crate.
/// This is required to generate the bindings of static C functions.
fn compile_extern_source_code() {
    let output_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // This is the path to the object file.
    let obj_path = output_path.join("extern.o");
    // This is the path to the static library file.
    let lib_path = output_path.join("libextern.a");
    
    // Copy header file to folder
    fs::copy(
        LINK_NAME,
        std::env::temp_dir().join("bindgen").join(LINK_NAME),
    )
    .expect("Failed to copy header file");
    
    // Compile the generated wrappers into an object file.
    let clang_output = std::process::Command::new("clang")
        .arg("-O")
        .arg("-c")
        .arg("-o")
        .arg(&obj_path)
        .arg(std::env::temp_dir().join("bindgen").join("extern.c"))
        .arg("-include")
        .arg(LINK_NAME)
        .output()
        .unwrap();
    
    if !clang_output.status.success() {
        panic!(
            "Could not compile object file:\n{}",
            String::from_utf8_lossy(&clang_output.stderr)
        );
    }
    
    // Turn the object file into a static library
    let lib_output = Command::new("ar")
        .arg("rcs")
        .arg(lib_path)
        .arg(obj_path)
        .output()
        .unwrap();
    
    if !lib_output.status.success() {
        panic!("Could not emit library file:\n");
    }
    
    // Tell cargo to statically link against the `libextern` static library.
    println!("cargo:rustc-link-search={}", env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-link-lib=static=extern");
}
