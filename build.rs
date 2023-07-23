use std::env;
use std::path::PathBuf;

fn main() {    
    // Tell cargo to look for shared libraries in the specified directory.
    println!("cargo:rustc-link-search=/usr/local/lib/");
    println!("cargo:rustc-link-search=/usr/lib/x86_64-linux-gnu/");
    
    // Tell cargo to tell rustc to link the system shared libraries.
    println!("cargo:rustc-link-lib=static=nethuns");
    println!("cargo:rustc-link-lib=pcap");
    println!("cargo:rustc-link-lib=netmap");
    
    // Tell cargo to invalidate the built crate whenever the wrapper changes.
    println!("cargo:rerun-if-changed=wrapper.h");
    
    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        // Use netmap as underlying I/O framework for Nethuns
        .clang_arg("-DNETHUNS_SOCKET=1")
        // Blacklist unused function which throws improper_ctypes warnings
        .blocklist_function("strtold")
        .blocklist_function("qecvt")
        .blocklist_function("qfcvt")
        .blocklist_function("qgcvt")
        .blocklist_function("qecvt_r")
        .blocklist_function("qfcvt_r")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
