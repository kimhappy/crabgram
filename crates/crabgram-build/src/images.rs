//! Checks the encoding of the styling's images, and their size against the framing of each image.

use std::{
    collections::BTreeMap,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use crabgram_styling::{Framing, Sprite, host};

use crate::BundleProblem;

const fn color_name(color: png::ColorType) -> &'static str {
    match color {
        png::ColorType::Grayscale => "grayscale",
        png::ColorType::Rgb => "RGB",
        png::ColorType::Indexed => "indexed",
        png::ColorType::GrayscaleAlpha => "grayscale with alpha",
        png::ColorType::Rgba => "RGBA",
    }
}

/// The width and height of the PNG `file`, with the problem of an encoding other than 8-bit RGBA.
fn header(file: &Path) -> Result<((u32, u32), Option<BundleProblem>), BundleProblem> {
    let reader = File::open(file).map_err(|error| BundleProblem::ImageUnopenable {
        file: file.to_owned(),
        error: error.to_string(),
    })?;
    let decoder = png::Decoder::new(BufReader::new(reader));
    let info = decoder.read_info().map_err(|error| BundleProblem::NotPng {
        file: file.to_owned(),
        error: error.to_string(),
    })?;
    let header = info.info();
    let not_rgba8 = (header.color_type != png::ColorType::Rgba
        || header.bit_depth != png::BitDepth::Eight)
        .then(|| BundleProblem::NotRgba8 {
            file: file.to_owned(),
            color: color_name(header.color_type),
            depth: header.bit_depth as u8,
        });

    Ok((header.size(), not_rgba8))
}

/// Whether a frame of `sprite`, cut from an image of `size`, reaches past the block image.
const fn overflows(sprite: &Sprite, size: (u32, u32)) -> bool {
    let (width, height) = sprite.frame().within(size);
    let (x, y) = sprite.position();

    x.saturating_add(width) > host::BLOCK_IMAGE || y.saturating_add(height) > host::BLOCK_IMAGE
}

fn violations<'a>(
    path: &str,
    framings: &[Framing],
    sprites: impl Iterator<Item = &'a Sprite>,
    locate: &impl Fn(&str) -> Result<PathBuf, BundleProblem>,
) -> Result<Vec<BundleProblem>, BundleProblem> {
    let file = locate(path)?;

    if !file.is_file() {
        return Err(BundleProblem::MissingImage {
            path: path.to_owned(),
            file,
        });
    }

    let (size, not_rgba8) = header(&file)?;
    let misframed = framings
        .iter()
        .filter_map(|framing| framing.mismatch(size))
        .map(|mismatch| BundleProblem::WrongFraming {
            path: path.to_owned(),
            mismatch,
        });

    let overflowing = sprites
        .filter(|sprite| overflows(sprite, size))
        .map(|sprite| BundleProblem::SpriteOutsideBlock {
            path: path.to_owned(),
            position: sprite.position(),
            frame: sprite.frame().within(size),
        });

    Ok(not_rgba8
        .into_iter()
        .chain(misframed)
        .chain(overflowing)
        .collect())
}

/// The images `locate` cannot find, that are no 8-bit RGBA PNG, that break a framing or whose
/// `sprites` reach past the block image, each file looked at once.
pub fn problems(
    images: &BTreeMap<&str, Vec<Framing>>,
    sprites: &[&Sprite],
    locate: impl Fn(&str) -> Result<PathBuf, BundleProblem>,
) -> Vec<BundleProblem> {
    images
        .iter()
        .flat_map(|(path, framings)| {
            let placed = sprites.iter().copied().filter(|it| it.path() == *path);

            violations(path, framings, placed, &locate).unwrap_or_else(|problem| vec![problem])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, io::BufWriter, num::NonZeroU32};

    use crabgram_styling::{Frame, Framing, Sprite};

    use super::problems;

    fn write_png(
        path: &std::path::Path,
        (width, height): (u32, u32),
        color: png::ColorType,
        depth: png::BitDepth,
    ) {
        let file = BufWriter::new(fs::File::create(path).unwrap());
        let mut encoder = png::Encoder::new(file, width, height);

        encoder.set_color(color);
        encoder.set_depth(depth);

        let mut writer = encoder.write_header().unwrap();
        let bytes = (width * height) as usize * color.samples() * usize::from(depth as u8) / 8;

        writer.write_image_data(&vec![0; bytes]).unwrap();
    }

    #[test]
    fn only_an_eight_bit_rgba_image_is_sound() {
        let directory = std::env::temp_dir().join(format!("crabgram-rgba-{}", std::process::id()));
        let encodings = [
            ("rgba.png", png::ColorType::Rgba, png::BitDepth::Eight),
            ("rgb.png", png::ColorType::Rgb, png::BitDepth::Eight),
            ("deep.png", png::ColorType::Rgba, png::BitDepth::Sixteen),
        ];

        fs::create_dir_all(&directory).unwrap();
        for (name, color, depth) in encodings {
            write_png(&directory.join(name), (2, 1), color, depth);
        }

        let images: BTreeMap<_, _> = encodings
            .map(|(name, ..)| (name, vec![Framing::image((2, 1))]))
            .into();
        let found: Vec<String> = problems(&images, &[], |path| Ok(directory.join(path)))
            .iter()
            .map(|problem| {
                problem
                    .to_string()
                    .replace(&directory.display().to_string(), "")
            })
            .collect();

        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(
            found,
            [
                "/deep.png is 16-bit RGBA, but every image must be 8-bit RGBA",
                "/rgb.png is 8-bit RGB, but every image must be 8-bit RGBA",
            ]
        );
    }

    #[test]
    fn a_sprite_must_stay_inside_the_block_image() {
        let directory =
            std::env::temp_dir().join(format!("crabgram-sprite-{}", std::process::id()));
        let lfo = |x, y| {
            Sprite::new("lfo.png", Frame::horizontal(NonZeroU32::new(60).unwrap()))
                .with_position(x, y)
        };
        let [inside, right, below] = [lfo(140, 140), lfo(150, 0), lfo(0, 141)];

        fs::create_dir_all(&directory).unwrap();
        write_png(
            &directory.join("lfo.png"),
            (120, 60),
            png::ColorType::Rgba,
            png::BitDepth::Eight,
        );

        let images = [("lfo.png", vec![Framing::Free])].into();
        let found: Vec<String> = problems(&images, &[&inside, &right, &below], |path| {
            Ok(directory.join(path))
        })
        .iter()
        .map(ToString::to_string)
        .collect();

        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(
            found,
            [
                r#""lfo.png" puts a 60x60 frame at 150, 0, which reaches past the 200x200 block image"#,
                r#""lfo.png" puts a 60x60 frame at 0, 141, which reaches past the 200x200 block image"#,
            ]
        );
    }
}
