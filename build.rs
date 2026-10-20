use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let output = Command::new("pkg-config")
        .args(["--libs", "gtk+-3.0", "gtk-layer-shell-0"])
        .output()
        .expect("failed to execute pkg-config for gtk+-3.0 and gtk-layer-shell-0");

    if !output.status.success() {
        panic!(
            "pkg-config failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let flags = String::from_utf8_lossy(&output.stdout);
    for token in flags.split_whitespace() {
        if let Some(lib) = token.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={lib}");
        } else if let Some(dir) = token.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={dir}");
        }
    }
}
