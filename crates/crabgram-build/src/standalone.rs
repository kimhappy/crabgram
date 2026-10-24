//! Builds the standalone app of a native bundle from the `crabgram-standalone` beside this crate.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    Error, Project, Target, copy, info, io_error, package,
    tools::{self, Workspace},
};

const MANIFEST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crabgram-standalone/Cargo.toml"
);

/// The app looks for the plugin, `ui.json` and the images beside itself.
pub fn install<P: Project>(workspace: &Workspace, directory: &Path) -> Result<(), Error> {
    copy(&build::<P>(workspace)?, &directory.join(package::<P>()))
}

/// Builds in its own workspace, with `CRABGRAM_PLUGIN` naming the plugin's library and
/// `CRABGRAM_PACKAGE` and `CRABGRAM_VERSION` the app.
pub fn build<P: Project>(workspace: &Workspace) -> Result<PathBuf, Error> {
    let standalone = Workspace::of(&manifest(Path::new(MANIFEST))?)?;

    Ok(tools::build_executable(
        Command::new(tools::cargo()?)
            .current_dir(&standalone.root)
            .env("CRABGRAM_PLUGIN", Target::Native.binary::<P>())
            .env("CRABGRAM_PACKAGE", package::<P>())
            .env("CRABGRAM_VERSION", info::<P>().version())
            .args(["build", "--release", "--package", "crabgram-standalone"])
            .arg("--target-dir")
            .arg(&workspace.target),
        standalone.package_id("crabgram-standalone")?,
    )?)
}

/// `manifest` with symbolic links resolved, as cargo reports canonical paths.
fn manifest(manifest: &Path) -> Result<PathBuf, Error> {
    manifest
        .canonicalize()
        .map_err(io_error("finding the standalone app at", manifest))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{MANIFEST, manifest};
    use crate::tools::Workspace;

    #[test]
    fn the_standalone_app_is_a_package_of_the_workspace_beside_this_crate() {
        let standalone =
            Workspace::of(Path::new(MANIFEST)).expect("cargo metadata of the crabgram workspace");

        assert!(standalone.package_id("crabgram-standalone").is_ok());
        assert!(
            standalone
                .root
                .join("crates/crabgram-standalone/Cargo.toml")
                .is_file()
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_standalone_app_is_found_through_a_symbolic_link() {
        let link = std::env::temp_dir().join(format!("crabgram-linked-{}", std::process::id()));
        std::os::unix::fs::symlink(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."), &link)
            .unwrap();

        let linked = manifest(&link.join("crates/crabgram-standalone/Cargo.toml"))
            .and_then(|manifest| Ok(Workspace::of(&manifest)?));

        fs::remove_file(&link).unwrap();

        let direct = Workspace::of(&manifest(Path::new(MANIFEST)).unwrap()).unwrap();

        assert_eq!(
            linked.unwrap().package_id("crabgram-standalone").unwrap(),
            direct.package_id("crabgram-standalone").unwrap()
        );
    }

    #[test]
    fn a_missing_standalone_app_names_the_manifest_it_looked_for() {
        let error = manifest(Path::new("/nonexistent/crabgram-standalone/Cargo.toml"))
            .unwrap_err()
            .to_string();

        assert_eq!(
            error,
            "finding the standalone app at /nonexistent/crabgram-standalone/Cargo.toml"
        );
    }
}
