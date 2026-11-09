//! On Linux, adds the directories of the libraries the app opens at run time to `RUNPATH`.

use std::{collections::BTreeSet, env, process::Command};

const LINUX_LIBRARIES: [&str; 10] = [
    "alsa",
    "wayland-client",
    "x11",
    "x11-xcb",
    "xcb",
    "xcursor",
    "xi",
    "xkbcommon",
    "xkbcommon-x11",
    "xrandr",
];

fn libdir(package: &str) -> String {
    let output = Command::new("pkg-config")
        .args(["--variable=libdir", package])
        .output()
        .expect("pkg-config is missing: build inside `nix develop`");

    assert!(
        output.status.success(),
        "pkg-config does not know {package}: build inside `nix develop`"
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn link_linux_libraries() {
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR");

    LINUX_LIBRARIES
        .into_iter()
        .map(libdir)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .for_each(|libdir| println!("cargo:rustc-link-arg=-Wl,-rpath,{libdir}"));
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        link_linux_libraries();
    }
}
