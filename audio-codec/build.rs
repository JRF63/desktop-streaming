use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    let header = "opus/include/opus.h";

    if !Path::new(header).exists() {
        panic!(
            "`opus.h` could not be found.\n\n\
            Try the command `git submodule update --init --recursive`."
        )
    }

    let bindings = bindgen::Builder::default()
        .header(header)
        .layout_tests(false)
        .generate_comments(false)
        .default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
        .generate()
        .expect("Unable to generate bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_name = "opus.rs";
    bindings
        .write_to_file(out_dir.join(out_name))
        .expect("Could not write bindings");

    let dst = cmake::build("opus");
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=opus");
}
