use std::{env, path::PathBuf, process::Command};

fn run(command: &mut Command) {
    assert!(
        command
            .status()
            .expect("starting native font build")
            .success(),
        "native font build failed"
    );
}

fn main() {
    println!("cargo:rerun-if-changed=native/font.c");
    println!("cargo:rerun-if-changed=scripts/build-font-libs.sh");
    for key in [
        "WORKSPACE_INDICATOR_FONT_PREFIX",
        "WORKSPACE_INDICATOR_FONT_CC",
        "ZIG",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let target = env::var("TARGET").expect("Cargo sets TARGET");
    let arch = target.split('-').next().expect("target architecture");
    let abi = if target.ends_with("-musl") {
        "musl"
    } else {
        "gnu"
    };
    let root = out.join("native");
    let prefix = env::var_os("WORKSPACE_INDICATOR_FONT_PREFIX")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            run(Command::new("bash")
                .arg("scripts/build-font-libs.sh")
                .arg(arch)
                .arg(&root)
                .arg(abi));
            root.join(format!("{arch}-{abi}/prefix"))
        });
    let cc = env::var_os("WORKSPACE_INDICATOR_FONT_CC")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(format!("{arch}-{abi}/bin/cc")));
    let object = out.join("font.o");
    run(Command::new(cc)
        .args([
            "-O2",
            "-fPIC",
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-c",
            "native/font.c",
        ])
        .arg(format!("-I{}/include", prefix.display()))
        .arg(format!("-I{}/include/freetype2", prefix.display()))
        .arg("-o")
        .arg(&object)
        .env("ZIG_GLOBAL_CACHE_DIR", out.join("zig-cache"))
        .env("ZIG_LOCAL_CACHE_DIR", out.join("zig-local-cache")));
    run(Command::new("ar")
        .arg("crs")
        .arg(out.join("libworkspace_font.a"))
        .arg(object));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=workspace_font");
    println!("cargo:rustc-link-search=native={}/lib", prefix.display());
    for library in ["fontconfig", "freetype", "expat"] {
        println!("cargo:rustc-link-lib=static={library}");
    }
    // musl includes both in libc; GNU development builds still need these.
    if abi == "gnu" {
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
    }
}
