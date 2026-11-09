//! With the `licensing` feature, links the license library and binds its header.

fn main() {
    #[cfg(feature = "licensing")]
    {
        println!("cargo::rerun-if-env-changed=NICKEL_LIB_DIR");
        println!("cargo::rerun-if-env-changed=NICKEL_INCLUDE_DIR");
        println!("cargo::rerun-if-env-changed=LV2_INCLUDE_DIR");
        licensing::link();
        licensing::bind();
    }
}

#[cfg(feature = "licensing")]
mod licensing {
    use std::{env, path::PathBuf};

    pub fn link() {
        let directory = env::var_os("NICKEL_LIB_DIR").map_or_else(
            || panic!("the licensing feature needs $NICKEL_LIB_DIR"),
            PathBuf::from,
        );

        println!(
            "cargo::rerun-if-changed={}",
            directory.join("libnickel.a").display()
        );
        println!("cargo::rustc-link-search=native={}", directory.display());
        println!("cargo::rustc-link-lib=static=nickel");
    }

    pub fn bind() {
        let include = |name: &str| {
            env::var_os(name).map_or_else(
                || panic!("the licensing feature needs ${name}"),
                PathBuf::from,
            )
        };
        let nickel = include("NICKEL_INCLUDE_DIR");
        let header = nickel.join("libnickel.h");
        let source = std::fs::read_to_string(&header)
            .expect("the header in $NICKEL_INCLUDE_DIR is readable");
        let visible = source
            .lines()
            .filter(|line| !line.starts_with("#pragma GCC visibility"))
            .collect::<Vec<_>>()
            .join("\n");

        println!("cargo::rerun-if-changed={}", header.display());

        bindgen::Builder::default()
            .header_contents("libnickel.h", &visible)
            .clang_arg(format!("-I{}", nickel.display()))
            .clang_arg(format!("-I{}", include("LV2_INCLUDE_DIR").display()))
            .clang_args(["-x", "c", "-ffreestanding"])
            .allowlist_function("nickel_.*")
            .blocklist_type("LV2_Feature")
            .raw_line("use crate::runtime::lv2::Feature as LV2_Feature;")
            .generate_comments(false)
            .layout_tests(false)
            .generate()
            .expect("the header binds")
            .write_to_file(
                PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("nickel.rs"),
            )
            .expect("the binding is written");
    }
}
