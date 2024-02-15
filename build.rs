use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-search=/usr/include");
    println!("cargo:rerun-if-changed=sqlite3ext.h");
    println!("cargo:rustc-link-lib=sqlite3");

    let bindings = bindgen::Builder::default()
        .header("/usr/include/sqlite3ext.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Unable to write bindings!");
}
