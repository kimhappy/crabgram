//! Checks an assembled bundle against the plugin, and for the device against the LV2 tools.

use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use crabgram_styling::Styling;

use crate::{
    Error, Project, Target, abi, assembled, build, files, info, io_error, package,
    problem::ensure_sound,
    standalone,
    tools::{self, Workspace, run, tool},
};

fn stale(directory: &Path, files: &[(String, Vec<u8>)]) -> Vec<String> {
    files
        .iter()
        .filter(|(name, contents)| fs::read(directory.join(name)).ok().as_ref() != Some(contents))
        .map(|(name, _)| name.clone())
        .collect()
}

fn ensure_fresh<P: Project>(
    styling: &Styling,
    directory: &Path,
    target: &Target,
) -> Result<(), Error> {
    let files = stale(directory, &files::<P>(styling, target)?);

    if files.is_empty() {
        Ok(())
    } else {
        Err(Error::Stale {
            directory: directory.to_owned(),
            files,
        })
    }
}

fn ensure_newer(built: &Path, bundled: PathBuf) -> Result<(), Error> {
    let modified = |path: &Path| fs::metadata(path).and_then(|it| it.modified());
    let built_at = modified(built).map_err(io_error("reading the age of", built))?;

    match modified(&bundled) {
        Err(_) => Err(Error::MissingBinary { binary: bundled }),
        Ok(bundled_at) if built_at > bundled_at => Err(Error::StaleBinary { binary: bundled }),
        Ok(_) => Ok(()),
    }
}

/// That the Turtle files or `ui.json`, the images, the binary and the standalone app of the bundle
/// at `directory` still match the plugin, which this builds.
fn ensure_current<P: Project>(
    workspace: &Workspace,
    styling: &Styling,
    directory: &Path,
    target: &Target,
) -> Result<(), Error> {
    ensure_fresh::<P>(styling, directory, target)?;
    ensure_newer(
        &build::<P>(workspace, target)?,
        directory.join(target.binary::<P>()),
    )?;

    match target {
        Target::Native => ensure_newer(
            &standalone::build::<P>(workspace)?,
            directory.join(package::<P>()),
        ),
        Target::Device { .. } => Ok(()),
    }
}

fn children(directory: &Path) -> Result<BTreeSet<PathBuf>, Error> {
    fs::read_dir(directory)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|it| it.path()))
                .collect::<io::Result<_>>()
        })
        .map_err(io_error("reading", directory))
}

fn turtles(directory: &Path) -> Result<BTreeSet<PathBuf>, Error> {
    Ok(children(directory)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|it| it == "ttl"))
        .collect())
}

fn validate(directory: &Path) -> Result<(), Error> {
    let bundles = tools::directory("LV2_BUNDLES")?;
    let validator = tool("SORD_VALIDATE_TOOL")?;
    let ontologies = children(&bundles)?
        .into_iter()
        .filter(|ontology| ontology.is_dir())
        .map(|ontology| turtles(&ontology))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(run(Command::new(validator)
        .args(ontologies.into_iter().flatten())
        .args(turtles(directory)?))?)
}

/// Lints the bundle at `directory`, which `-I` loads, against the ontologies alone: another bundle
/// with the same URI must not stand in for it.
fn lint(directory: &Path, uri: &str) -> Result<(), Error> {
    Ok(run(tools::emulated(&tool("LV2LINT_TOOL")?)?
        .env("LV2_PATH", tools::directory("LV2_BUNDLES")?)
        .args(["-E", "warn", "-I"])
        .arg(directory)
        .arg(uri))?)
}

/// The bundle of `target`, checked against the plugin; for the device, also against the Turtle
/// validator, `lv2lint` and the ABI.
pub fn checked<P: Project>(workspace: &Workspace, target: &Target) -> Result<PathBuf, Error> {
    let directory = assembled::<P>(workspace, target)?;
    let styling = P::styling();

    ensure_sound::<P>(&styling)?;
    ensure_current::<P>(workspace, &styling, &directory, target)?;

    if let Target::Device { .. } = target {
        validate(&directory)?;
        lint(&directory, info::<P>().uri())?;
        abi::check(&directory.join(target.binary::<P>()))?;
    }

    Ok(directory)
}

pub fn check<P: Project>() -> Result<(), Error> {
    let directory = checked::<P>(&Workspace::load()?, &Target::current())?;

    println!("checked {}", directory.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use crabgram_styling::Styling;

    use super::{Error, ensure_fresh, ensure_newer, stale, turtles};
    use crate::{
        Project, Target, descriptions,
        fixture::{Styled, Xtask},
        ui, write_files,
    };

    #[test]
    fn a_bundled_binary_is_missing_stale_or_current() {
        let directory =
            std::env::temp_dir().join(format!("crabgram-binary-age-{}", std::process::id()));
        let [built, bundled] = ["built.so", "bundled.so"].map(|name| directory.join(name));
        let age = |path: &Path, seconds| {
            fs::File::options()
                .write(true)
                .open(path)
                .and_then(|file| {
                    file.set_modified(
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
                    )
                })
                .unwrap();
        };

        fs::create_dir_all(&directory).unwrap();

        let outcome = || match ensure_newer(&built, bundled.clone()) {
            Ok(()) => "current",
            Err(Error::Io { .. }) => "unbuilt",
            Err(Error::MissingBinary { .. }) => "missing",
            Err(Error::StaleBinary { .. }) => "stale",
            Err(_) => "other",
        };
        let unbuilt = outcome();

        fs::write(&built, "").unwrap();

        let missing = outcome();

        fs::write(&bundled, "").unwrap();
        age(&built, 2_000);
        age(&bundled, 1_000);

        let stale = outcome();

        age(&bundled, 3_000);

        let current = outcome();

        fs::remove_dir_all(&directory).unwrap();

        assert_eq!(
            [unbuilt, missing, stale, current],
            ["unbuilt", "missing", "stale", "current"]
        );
    }

    #[test]
    fn an_output_that_no_longer_describes_the_plugin_is_stale() {
        let directory = std::env::temp_dir().join(format!("crabgram-stale-{}", std::process::id()));
        let styling = Styling::default();

        fs::create_dir_all(&directory).unwrap();

        let device = Target::Device {
            triple: "aarch64-unknown-linux-gnu".to_owned(),
        };
        let staleness = [device, Target::Native].map(|target| {
            let descriptions = descriptions::<Xtask>(&styling, &target).unwrap();

            write_files::<Xtask>(&styling, &directory, &target).unwrap();

            let fresh = stale(&directory, &descriptions);
            let (name, _) = &descriptions[0];

            fs::write(directory.join(name), "old").unwrap();

            let edited = stale(&directory, &descriptions);

            fs::remove_file(directory.join(name)).unwrap();
            (fresh, edited, stale(&directory, &descriptions))
        });

        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(
            staleness.map(|(fresh, edited, missing)| (fresh.len(), edited, missing)),
            [
                (
                    0,
                    vec!["manifest.ttl".to_owned()],
                    vec!["manifest.ttl".to_owned()]
                ),
                (0, vec![ui::FILE.to_owned()], vec![ui::FILE.to_owned()]),
            ]
        );
    }

    #[test]
    fn a_stale_bundle_is_refused_with_the_files_it_lacks_or_holds_otherwise() {
        let directory = std::env::temp_dir().join(format!("crabgram-fresh-{}", std::process::id()));
        let styling = Styling::default();

        let device = Target::Device {
            triple: "aarch64-unknown-linux-gnu".to_owned(),
        };

        fs::create_dir_all(&directory).unwrap();
        write_files::<Xtask>(&styling, &directory, &device).unwrap();

        let fresh = ensure_fresh::<Xtask>(&styling, &directory, &device);

        fs::remove_file(directory.join("manifest.ttl")).unwrap();

        let stale = ensure_fresh::<Xtask>(&styling, &directory, &device);

        fs::remove_dir_all(&directory).unwrap();
        assert!(fresh.is_ok());
        assert_eq!(
            stale.unwrap_err().to_string(),
            format!(
                "the bundle at {} no longer matches the plugin (manifest.ttl missing or changed): \
                 bundle it again",
                directory.display()
            )
        );
    }

    #[test]
    fn a_bundle_whose_image_changed_is_stale() {
        let directory = std::env::temp_dir().join(format!("crabgram-image-{}", std::process::id()));
        let knob = Path::new(Styled::KNOB);
        let styling = Styled::styling();

        fs::create_dir_all(knob.parent().unwrap()).unwrap();
        fs::create_dir_all(&directory).unwrap();
        fs::write(knob, "old").unwrap();
        write_files::<Styled>(&styling, &directory, &Target::Native).unwrap();

        let fresh = ensure_fresh::<Styled>(&styling, &directory, &Target::Native);

        fs::write(knob, "new").unwrap();

        let stale = ensure_fresh::<Styled>(&styling, &directory, &Target::Native);

        fs::remove_dir_all(&directory).unwrap();
        assert!(fresh.is_ok());
        assert!(
            matches!(stale, Err(Error::Stale { ref files, .. }) if files == &["asset/knob.png"]),
            "{stale:?}"
        );
    }

    #[test]
    fn only_the_turtle_files_of_a_directory_are_listed() {
        let directory =
            std::env::temp_dir().join(format!("crabgram-turtles-{}", std::process::id()));

        fs::create_dir_all(&directory).unwrap();
        for name in ["manifest.ttl", "gain.ttl", "gain.so", "notes"] {
            fs::write(directory.join(name), "").unwrap();
        }

        let found = turtles(&directory).unwrap();

        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            [directory.join("gain.ttl"), directory.join("manifest.ttl")]
        );
        assert!(turtles(&directory).is_err());
    }
}
