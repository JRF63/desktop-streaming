fn main() {
    // https://github.com/rust-windowing/android-ndk-rs/blob/cargo-apk-0.9.1/cargo-apk/src/apk.rs#L184
    println!("cargo:rustc-link-search=./gcc");
}
