//! What is wrong with the styling, its images and the plugin description, found without building.

use core::fmt;
use std::{collections::BTreeMap, error::Report, path::PathBuf};

use crabgram_styling::{BlockImage, Framing, Mismatch, PathReason, Sprite, Styling};

use crate::{
    ASSET_DIRECTORY, Builtin, Error, Project,
    assets::{asset_file, unnamed},
    controls, images, package, prepare, prepare_of, turtle,
};

/// Something wrong with the styling, its images or the plugin description, from [`problems`].
#[derive(Clone, Debug, Eq, PartialEq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum BundleProblem {
    /// A mistake in the styling itself.
    #[error(transparent)]
    Styling(#[from] crabgram_styling::Problem),
    /// A package named like another file or directory of the bundle, which its description,
    /// `<package>.ttl`, or the standalone app, `<package>`, would be.
    #[error("the package {package:?} takes the name of {file}, which the bundle needs for itself")]
    ReservedPackage {
        /// The package.
        package: &'static str,
        /// The file or directory both would be.
        file: &'static str,
    },
    /// The styling names an image, but [`Project::ASSETS`] is `None`.
    #[error("the styling names {path:?}, but the project has no asset directory")]
    NoAssetDirectory {
        /// The styling path.
        path: String,
    },
    /// A styling path outside [`ASSET_DIRECTORY`].
    #[error(
        "{path:?} lies outside {ASSET_DIRECTORY}/, the only directory of images the bundle ships"
    )]
    OutsideAssetDirectory {
        /// The styling path.
        path: String,
    },
    /// The [`Prepare`](crate::Prepare) step of the assets failed.
    #[error("{error}")]
    PrepareFailed {
        /// What failed, with its causes.
        error: String,
    },
    /// A styling path names the asset directory itself instead of an image in it.
    #[error("{path:?} names {ASSET_DIRECTORY}/ itself, not an image in it")]
    NamesAssetDirectory {
        /// The styling path.
        path: String,
    },
    /// The file a styling path names does not exist.
    #[error("the styling names {path:?}, but {} does not exist", file.display())]
    MissingImage {
        /// The styling path.
        path: String,
        /// The file it names.
        file: PathBuf,
    },
    /// An image that cannot be opened.
    #[error("opening {}: {error}", file.display())]
    ImageUnopenable {
        /// The image file.
        file: PathBuf,
        /// Why it cannot be opened.
        error: String,
    },
    /// An image that is no PNG.
    #[error("{} is no PNG: {error}", file.display())]
    NotPng {
        /// The image file.
        file: PathBuf,
        /// What the decoder found.
        error: String,
    },
    /// An image encoded other than as 8-bit RGBA.
    #[error("{} is {depth}-bit {color}, but every image must be 8-bit RGBA", file.display())]
    NotRgba8 {
        /// The image file.
        file: PathBuf,
        /// Its color type.
        color: &'static str,
        /// Its bits per sample.
        depth: u8,
    },
    /// An image whose size breaks a framing it must fit.
    #[error("{path:?} is {mismatch}")]
    WrongFraming {
        /// The styling path.
        path: String,
        /// The size and the framing it breaks.
        mismatch: Mismatch,
    },
    /// A block image sprite whose frame reaches past the
    /// [`BLOCK_IMAGE`](crabgram_styling::host::BLOCK_IMAGE) square.
    #[error(
        "{path:?} puts a {}x{} frame at {}, {}, which reaches past the {block}x{block} block image",
        frame.0,
        frame.1,
        position.0,
        position.1,
        block = crabgram_styling::host::BLOCK_IMAGE
    )]
    SpriteOutsideBlock {
        /// The styling path of the sprite.
        path: String,
        /// The left and top edge of the sprite in the block image.
        position: (u32, u32),
        /// The width and height of one frame of the sprite.
        frame: (u32, u32),
    },
    /// The asset directory, or one below it, cannot be read.
    #[error("reading {}: {error}", directory.display())]
    AssetDirectoryUnreadable {
        /// The directory.
        directory: PathBuf,
        /// Why it cannot be read.
        error: String,
    },
    /// A file of the asset directory that the styling never names.
    #[error("{} is in the asset directory, but the styling never names it", file.display())]
    UnnamedAsset {
        /// The file as the bundle ships it, under [`ASSET_DIRECTORY`].
        file: PathBuf,
    },
}

/// The [`BundleProblem`]s of a project, one per line when displayed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Problems(Vec<BundleProblem>);

impl Problems {
    /// Whether there are none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Each problem, in the order found.
    pub fn iter(&self) -> impl Iterator<Item = &BundleProblem> {
        self.0.iter()
    }
}

impl FromIterator<BundleProblem> for Problems {
    fn from_iter<I: IntoIterator<Item = BundleProblem>>(problems: I) -> Self {
        Self(problems.into_iter().collect())
    }
}

impl fmt::Display for Problems {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.iter().enumerate().try_for_each(|(position, problem)| {
            let separator = if position == 0 { "" } else { "\n" };

            write!(formatter, "{separator}{problem}")
        })
    }
}

/// Refuses `styling` while [`problems_in`] finds any.
pub fn ensure_sound<P: Project>(styling: &Styling) -> Result<(), Error> {
    let problems = problems_in::<P>(styling);

    if problems.is_empty() {
        Ok(())
    } else {
        Err(Error::Unsound { problems })
    }
}

/// Everything wrong with the styling and images of `P`, found without building: `bundle` and
/// `check` refuse to run while there are any.
///
/// Runs the [`Prepare`](crate::Prepare) step of the assets first when `bundle` does.
///
/// Call it from a unit test of the xtask:
///
/// ```ignore
/// #[test]
/// fn the_images_fit_the_styling() {
///     let problems = crabgram_build::problems::<Xtask>();
///
///     assert!(problems.is_empty(), "{problems}");
/// }
/// ```
#[must_use]
pub fn problems<P: Project>() -> Problems {
    let prepared = prepare_of::<P>().is_some_and(|it| it.before.contains(&Builtin::Bundle));

    match if prepared { prepare::<P>() } else { Ok(()) } {
        Ok(()) => problems_in::<P>(&P::styling()),
        Err(error) => Problems(vec![BundleProblem::PrepareFailed {
            error: Report::new(error).to_string(),
        }]),
    }
}

/// Everything wrong with `styling` and its images.
pub fn problems_in<P: Project>(styling: &Styling) -> Problems {
    let controls = controls::<P>();
    let named = styling.images(controls);
    let images: BTreeMap<&str, Vec<Framing>> = named
        .iter()
        .filter(|(path, _)| PathReason::of(path).is_none())
        .map(|(&path, framings)| (path, framings.clone()))
        .collect();
    let sprites: Vec<&Sprite> = styling
        .block_image()
        .into_iter()
        .flat_map(BlockImage::sprites)
        .collect();
    let package = package::<P>();
    let reserved = [
        turtle::MANIFEST,
        crabgram_styling::turtle::FILE,
        ASSET_DIRECTORY,
    ]
    .into_iter()
    .filter(|file| file.strip_suffix(".ttl").unwrap_or(file) == package)
    .map(|file| BundleProblem::ReservedPackage { package, file });
    let assets = crate::asset_directory::<P>();
    let assets: Vec<BundleProblem> =
        images::problems(&images, &sprites, |path| asset_file(assets, path))
            .into_iter()
            .chain(unnamed(assets, named.keys().copied()))
            .collect();

    styling
        .problems(controls)
        .into_iter()
        .map(BundleProblem::from)
        .chain(reserved)
        .chain(assets)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{error::Report, fs, path::Path};

    use crabgram_styling::{BlockSettings, Framing, Parameter, Styling, Widget};

    use super::{BundleProblem, Problems, problems_in};
    use crate::{
        Error,
        fixture::{Knobbed, Params, Reserved, Xtask},
    };

    fn messages(problems: &Problems) -> Vec<String> {
        problems.iter().map(ToString::to_string).collect()
    }

    fn two_problems() -> Problems {
        [
            BundleProblem::NoAssetDirectory {
                path: "asset/knob.png".to_owned(),
            },
            BundleProblem::OutsideAssetDirectory {
                path: "knob.png".to_owned(),
            },
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn the_problems_show_one_per_line() {
        assert_eq!(
            two_problems().to_string(),
            "the styling names \"asset/knob.png\", but the project has no asset directory\n\
             \"knob.png\" lies outside asset/, the only directory of images the bundle ships"
        );
        assert_eq!(Problems::default().to_string(), "");
    }

    #[test]
    fn an_unsound_result_reports_every_problem() {
        let unsound = Error::Unsound {
            problems: two_problems(),
        };

        assert_eq!(
            Report::new(&unsound).to_string(),
            "the styling or its images are unsound:\n  the styling names \"asset/knob.png\", but \
             the project has no asset directory\n  \"knob.png\" lies outside asset/, the only \
             directory of images the bundle ships"
        );
    }

    #[test]
    fn a_wrong_framing_names_the_path_and_what_it_needs() {
        let problem = BundleProblem::WrongFraming {
            path: "asset/top-bar.png".to_owned(),
            mismatch: Framing::image((800, 60)).mismatch((800, 50)).unwrap(),
        };

        assert_eq!(
            problem.to_string(),
            r#""asset/top-bar.png" is 800x50, but it needs a 800x60 image"#
        );
    }

    #[test]
    fn every_issue_is_reported_once() {
        let knob = || Widget::new("asset/knob.png");
        let styling = Styling::default().with_block_settings(
            BlockSettings::default()
                .with_knob(knob())
                .with_parameter(Parameter::of(Params::GAIN).with_widget(knob())),
        );

        assert_eq!(
            messages(&problems_in::<Xtask>(&styling)),
            [r#"the styling names "asset/knob.png", but the project has no asset directory"#]
        );
    }

    #[test]
    fn a_package_named_like_a_file_of_the_bundle_is_refused() {
        assert_eq!(
            messages(&problems_in::<Reserved<1>>(&Styling::default())),
            [
                r#"the package "manifest" takes the name of manifest.ttl, which the bundle needs for itself"#
            ]
        );
        assert_eq!(
            messages(&problems_in::<Reserved<2>>(&Styling::default())),
            [r#"the package "asset" takes the name of asset, which the bundle needs for itself"#]
        );
    }

    #[test]
    fn a_file_named_by_a_path_the_styling_rejects_is_not_unnamed_too() {
        let directory = Path::new(Knobbed::DIRECTORY);
        let styling = Styling::default().with_block_settings(
            BlockSettings::default().with_knob(Widget::new("asset/./knob.png")),
        );

        fs::create_dir_all(directory).unwrap();
        fs::write(directory.join("knob.png"), "").unwrap();

        assert_eq!(
            messages(&problems_in::<Knobbed>(&styling)),
            [
                r#""asset/./knob.png" holds an empty or `.` component: name the file plainly, as asset/<file>"#
            ]
        );
    }

    #[test]
    fn a_path_the_styling_rejects_is_not_looked_up_in_the_asset_directory() {
        let styling = Styling::default()
            .with_block_settings(BlockSettings::default().with_background("../background.png"));

        assert_eq!(
            messages(&problems_in::<Xtask>(&styling)),
            [r#""../background.png" leaves the bundle"#]
        );
    }
}
