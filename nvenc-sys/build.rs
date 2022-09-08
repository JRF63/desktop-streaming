use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=headers/11_1/nvEncodeAPI.h");
    let bindings = bindgen::Builder::default()
        .header("headers/11_1/nvEncodeAPI.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .layout_tests(false)
        .derive_debug(true)
        .generate_comments(false)
        .default_enum_style(bindgen::EnumVariation::Rust { non_exhaustive: true })
        .rustified_enum("_NVENCSTATUS")
        .generate()
        .expect("Unable to generate bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}