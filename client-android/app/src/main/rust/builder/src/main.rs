use clap::Parser;
use ndk_build::{cargo::cargo_ndk, ndk::Ndk, target::Target};

#[derive(Parser, Debug)]
struct Args {
    /// Path of the crate to be built
    #[clap(parse(from_os_str))]
    path: std::path::PathBuf,

    /// Target ABI
    #[clap(parse(try_from_str = parse_abi))]
    target: Target,

    /// Minimum Android SDK version to be supported
    min_sdk_version: u32,

    /// Build artifacts in release mode, with optimizations
    #[clap(short, long)]
    release: bool,
}

#[derive(Debug)]
struct AbiParsingError(String);

impl std::fmt::Display for AbiParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unable to parse as a supported Android architecture: {}", self.0)
    }
}

impl std::error::Error for AbiParsingError {}

fn parse_abi(s: &str) -> Result<Target, AbiParsingError> {
    let s = s.to_string();
    let lowercase = s.to_ascii_lowercase();
    if lowercase == "armv7a" {
        Ok(Target::ArmV7a)
    } else if lowercase == "arm64v8a" {
        Ok(Target::Arm64V8a)
    } else if lowercase == "x86" {
        Ok(Target::X86)
    } else if lowercase == "x86_64" {
        Ok(Target::X86_64)
    } else {
        Err(AbiParsingError(s))
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let ndk = Ndk::from_env()?;

    // The env variables `CARGO_TARGET_<triple>_AR`, `AR_<triple>`, `CC_<triple>` and
    // `CXX_<triple>` does not seem to be used by anything.
    let mut cargo = cargo_ndk(&ndk, args.target, args.min_sdk_version)?;
    cargo.current_dir(&args.path);
    cargo.arg("build").arg("--target").arg(args.target.rust_triple());

    if args.release {
        cargo.arg("--release");
    }

    cargo.status()?;
    Ok(())
}
