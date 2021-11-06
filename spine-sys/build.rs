use std::env;
use std::path::PathBuf;

fn main() {
    let dst = cmake::build("spine-runtimes/spine-c");
    println!("cargo:rustc-link-search=native={}/dist/lib", dst.display());
    println!("cargo:rustc-link-lib=static=spine-c");

    println!("cargo:rerun-if-changed=wrapper.h");
    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}/spine-runtimes/spine-c/spine-c/include", env::var("CARGO_MANIFEST_DIR").unwrap()))
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
