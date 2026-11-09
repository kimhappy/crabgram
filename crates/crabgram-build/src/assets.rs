//! Where the styling's images live in the project, and which files of the asset directory it leaves
//! unnamed.

use std::{
    collections::HashSet,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use crabgram_styling::{PathReason, Problem};

use crate::{ASSET_DIRECTORY, BundleProblem};

fn in_asset_directory(path: &str) -> Option<&Path> {
    Path::new(path).strip_prefix(ASSET_DIRECTORY).ok()
}

pub fn asset_file(assets: Option<&Path>, path: &str) -> Result<PathBuf, BundleProblem> {
    let assets = assets.ok_or_else(|| BundleProblem::NoAssetDirectory {
        path: path.to_owned(),
    })?;

    if let Some(reason) = PathReason::of(path) {
        return Err(Problem::PathInvalid {
            path: path.to_owned(),
            reason,
        }
        .into());
    }

    let relative =
        in_asset_directory(path).ok_or_else(|| BundleProblem::OutsideAssetDirectory {
            path: path.to_owned(),
        })?;

    if relative.as_os_str().is_empty() {
        return Err(BundleProblem::NamesAssetDirectory {
            path: path.to_owned(),
        });
    }

    Ok(assets.join(relative))
}

/// Writes `bytes` to `<directory>/<file>` for the styling path `asset/<file>`, creating its
/// directories; a file that already holds `bytes` keeps its age.
///
/// A [`Prepare`](crate::Prepare) step that draws the images calls it for each:
///
/// ```
/// let assets = std::env::temp_dir().join(format!("crabgram-write-asset-{}", std::process::id()));
///
/// crabgram_build::write_asset(&assets, "asset/knob/big.png", b"png")?;
///
/// assert_eq!(std::fs::read(assets.join("knob/big.png"))?, b"png");
/// # std::fs::remove_dir_all(&assets)?;
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// # Errors
///
/// An [`InvalidInput`](io::ErrorKind::InvalidInput) error holding the [`BundleProblem`] when
/// `path` is invalid, lies outside [`ASSET_DIRECTORY`] or names it; otherwise the error of
/// creating or writing the file.
pub fn write_asset(directory: &Path, path: &str, bytes: &[u8]) -> io::Result<()> {
    let file = asset_file(Some(directory), path)
        .map_err(|problem| io::Error::new(io::ErrorKind::InvalidInput, problem))?;

    if fs::read(&file).is_ok_and(|it| it == bytes) {
        return Ok(());
    }

    fs::create_dir_all(file.parent().unwrap_or(directory))?;
    fs::write(file, bytes)
}

fn hidden(path: &Path) -> bool {
    path.file_name()
        .map(OsStr::as_encoded_bytes)
        .is_some_and(|it| it.starts_with(b"."))
}

/// Every file below `directory`, except hidden files and links to directories.
fn files(directory: &Path) -> Result<Vec<PathBuf>, BundleProblem> {
    let unreadable = |error: std::io::Error| BundleProblem::AssetDirectoryUnreadable {
        directory: directory.to_owned(),
        error: error.to_string(),
    };

    fs::read_dir(directory)
        .map_err(unreadable)?
        .map(|entry| {
            let entry = entry.map_err(unreadable)?;
            let path = entry.path();
            let kind = entry.file_type().map_err(unreadable)?;

            if hidden(&path) || (kind.is_symlink() && path.is_dir()) {
                Ok(Vec::new())
            } else if kind.is_dir() {
                files(&path)
            } else {
                Ok(vec![path])
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|it| it.concat())
}

/// The files of the asset directory that none of the styling `paths` names.
pub fn unnamed<'a>(
    assets: Option<&Path>,
    paths: impl IntoIterator<Item = &'a str>,
) -> Vec<BundleProblem> {
    let Some(directory) = assets else {
        return Vec::new();
    };
    let named: HashSet<PathBuf> = paths
        .into_iter()
        .filter_map(|path| Some(directory.join(in_asset_directory(path)?)))
        .collect();

    match files(directory) {
        Err(problem) => vec![problem],
        Ok(files) => files
            .into_iter()
            .filter(|file| !named.contains(file))
            .map(|file| BundleProblem::UnnamedAsset {
                file: Path::new(ASSET_DIRECTORY)
                    .join(file.strip_prefix(directory).unwrap_or(&file)),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use crabgram_styling::Problem;

    use super::{asset_file, unnamed, write_asset};
    use crate::BundleProblem;

    #[test]
    fn a_styling_path_names_a_file_of_the_asset_directory() {
        assert_eq!(
            asset_file(Some(Path::new("/assets")), "asset/knob/big.png").unwrap(),
            Path::new("/assets/knob/big.png")
        );
        assert!(matches!(
            asset_file(Some(Path::new("/assets")), "image/knob.png"),
            Err(BundleProblem::OutsideAssetDirectory { ref path }) if path == "image/knob.png"
        ));
        assert!(matches!(
            asset_file(Some(Path::new("/assets")), "asset"),
            Err(BundleProblem::NamesAssetDirectory { ref path }) if path == "asset"
        ));
        assert!(matches!(
            asset_file(Some(Path::new("/assets")), "asset/"),
            Err(BundleProblem::Styling(Problem::PathInvalid { ref path, .. })) if path == "asset/"
        ));
        assert!(matches!(
            asset_file(None, "asset/knob.png"),
            Err(BundleProblem::NoAssetDirectory { ref path }) if path == "asset/knob.png"
        ));
    }

    #[test]
    fn an_asset_path_that_leaves_the_directory_is_refused_before_writing() {
        let assets = std::env::temp_dir().join(format!("crabgram-escape-{}", std::process::id()));
        let escaped = write_asset(&assets, "asset/../escaped.png", b"png").unwrap_err();

        assert_eq!(escaped.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(
            escaped.to_string(),
            r#""asset/../escaped.png" leaves the bundle"#
        );
        assert!(!assets.exists());
        assert!(!std::env::temp_dir().join("escaped.png").exists());
    }

    #[test]
    fn an_asset_is_written_only_when_its_contents_change() {
        let assets = std::env::temp_dir().join(format!("crabgram-written-{}", std::process::id()));
        let file = assets.join("knob/big.png");
        let age = || fs::metadata(&file).and_then(|it| it.modified()).unwrap();

        write_asset(&assets, "asset/knob/big.png", b"old").unwrap();
        fs::File::options()
            .write(true)
            .open(&file)
            .and_then(|it| it.set_modified(std::time::UNIX_EPOCH))
            .unwrap();
        write_asset(&assets, "asset/knob/big.png", b"old").unwrap();

        let kept = age();

        write_asset(&assets, "asset/knob/big.png", b"new").unwrap();

        let contents = fs::read(&file).unwrap();
        let rewritten = age();
        let outside = write_asset(&assets, "image/knob.png", b"").unwrap_err();

        fs::remove_dir_all(&assets).unwrap();
        assert_eq!(kept, std::time::UNIX_EPOCH);
        assert_ne!(rewritten, std::time::UNIX_EPOCH);
        assert_eq!(contents, b"new");
        assert_eq!(outside.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(
            outside.to_string(),
            r#""image/knob.png" lies outside asset/, the only directory of images the bundle ships"#
        );
    }

    #[test]
    fn files_the_styling_never_names_are_reported() {
        let assets = std::env::temp_dir().join(format!("crabgram-unnamed-{}", std::process::id()));

        fs::create_dir_all(assets.join("knob")).unwrap();
        for file in ["knob.png", "knob/unused.png", ".hidden.png"] {
            fs::write(assets.join(file), b"").unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(assets.join("knob"), assets.join("linked")).unwrap();

        let found: Vec<String> = unnamed(Some(&assets), ["asset/knob.png"])
            .iter()
            .map(ToString::to_string)
            .collect();

        fs::remove_dir_all(&assets).unwrap();
        assert_eq!(
            found,
            ["asset/knob/unused.png is in the asset directory, but the styling never names it"]
        );
        assert_eq!(unnamed(None, []), []);
    }
}
