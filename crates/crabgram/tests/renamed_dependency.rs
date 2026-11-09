//! The macros follow a renamed `crabgram` dependency.

use std::{env, fs, path::Path, process::Command};

const LIB: &str = r#"
#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

use renamed::prelude::*;

#[derive(Debug, PartialEq, Choice)]
pub enum Mode {
    Boost,
    Cut,
}

#[derive(Controls)]
pub struct Params {
    #[control(default = 3.0, min = 0.1, max = 12.0, unit = Hertz, logarithmic, blockable)]
    pub rate: f32,
    pub mode: Mode,
}

pub struct Gain;

impl Plugin for Gain {
    type Controls = Params;

    const INFO: Info = renamed::info! {
        uri: c"urn:example:renamed",
        name: "Renamed",
        description: "",
        abbreviation: "RN",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _: &Params, context: &mut Context<'_, Self>) {
        let _ = context.set_state(Params::RATE, ControlState::Normal);
    }
}

export!(Gain);
"#;

fn manifest(crabgram: &Path) -> String {
    format!(
        r#"[package]
name = "renamed-dependent"
version = "0.1.0"
edition = "2024"
authors = ["Ada <ada@example.com>"]
repository = "https://example.com/renamed"
license = "MIT"

[lib]
crate-type = ["rlib"]

[dependencies]
renamed = {{ package = "crabgram", path = "{}" }}

[workspace]
"#,
        crabgram.display()
    )
}

#[test]
#[cfg_attr(miri, ignore = "runs cargo")]
fn a_renamed_dependency_expands_through_its_new_name() {
    let crabgram = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = Path::new(env!("CARGO_TARGET_TMPDIR")).join("renamed-dependent");

    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("Cargo.toml"), manifest(crabgram)).unwrap();
    fs::write(project.join("src/lib.rs"), LIB).unwrap();
    fs::copy(
        crabgram.join("../../Cargo.lock"),
        project.join("Cargo.lock"),
    )
    .unwrap();

    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--lib"])
        .current_dir(&project)
        .env("CARGO_TARGET_DIR", project.join("target"))
        .env("RUSTFLAGS", "-Znext-solver=coherence")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
