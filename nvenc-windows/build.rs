use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=nvEncodeAPI.h");
    let bindings = bindgen::Builder::default()
        .header("nvEncodeAPI.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .layout_tests(false)
        .rustified_enum("_NVENCSTATUS")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}