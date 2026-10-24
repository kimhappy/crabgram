//! Text drawn in the fonts the build embeds.

use std::{borrow::Cow, collections::HashMap, ops::Range, sync::LazyLock};

use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont, point};
use tiny_skia::{Color, PremultipliedColorU8};

use super::canvas::Canvas;

/// The fonts of the preview.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Face {
    /// Semibold condensed font for the captions of the strip's boxes.
    Heading,
    /// The regular font.
    Mono,
    /// The medium font, for the block name and the scene box.
    MonoMedium,
}

static FONTS: LazyLock<[FontArc; 3]> = LazyLock::new(|| {
    [
        include_bytes!(env!(
            "HEADING_FONT",
            "HEADING_FONT is unset: build inside `nix develop`"
        ))
        .as_slice(),
        include_bytes!(env!(
            "MONO_FONT",
            "MONO_FONT is unset: build inside `nix develop`"
        ))
        .as_slice(),
        include_bytes!(env!(
            "MONO_MEDIUM_FONT",
            "MONO_MEDIUM_FONT is unset: build inside `nix develop`"
        ))
        .as_slice(),
    ]
    .map(|bytes| FontArc::try_from_slice(bytes).expect("the embedded fonts are valid TTFs"))
});

impl Face {
    fn font(self) -> &'static FontArc {
        &FONTS[self as usize]
    }
}

/// The scale that makes one em `size` px: `PxScale` counts the whole line height, which is more
/// than an em.
fn em(face: Face, size: f32) -> PxScale {
    let font = face.font();
    let units = font.units_per_em().unwrap_or(1000.0);

    PxScale::from(size * font.height_unscaled() / units)
}

/// The width of `text` at an em of `size` px.
#[must_use]
pub fn measure(face: Face, size: f32, text: &str) -> f32 {
    width(face, em(face, size), text, 1.0)
}

/// `text`, or as much of it as fits in `width` followed by `...`, at an em of `size` px.
#[must_use]
pub fn fitted(face: Face, size: f32, text: &str, width: f32) -> Cow<'_, str> {
    const ELLIPSIS: &str = "...";

    if text.is_empty() || measure(face, size, text) <= width {
        return text.into();
    }

    let room = width - measure(face, size, ELLIPSIS);
    let ends: Vec<usize> = text.char_indices().map(|(end, _)| end).collect();
    let kept = ends.partition_point(|&end| measure(face, size, &text[..end]) <= room);

    format!("{}{ELLIPSIS}", &text[..ends[kept.saturating_sub(1)]]).into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HAlign {
    Left,
    Mid,
    Right,
}

/// Where [`draw`] puts a line of text: a box from `left` to `left + width`, aligned by `align`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    left: f32,
    width: f32,
    baseline: f32,
    align: HAlign,
}

impl Placement {
    const fn aligned(align: HAlign, left: f32, width: f32, baseline: f32) -> Self {
        Self {
            left,
            width,
            baseline,
            align,
        }
    }

    /// Text starting at `left`.
    #[must_use]
    pub const fn left(left: f32, width: f32, baseline: f32) -> Self {
        Self::aligned(HAlign::Left, left, width, baseline)
    }

    /// Text centred in the box from `left` to `left + width`.
    #[must_use]
    pub const fn mid(left: f32, width: f32, baseline: f32) -> Self {
        Self::aligned(HAlign::Mid, left, width, baseline)
    }

    /// Text ending at `left + width`.
    #[must_use]
    pub const fn right(left: f32, width: f32, baseline: f32) -> Self {
        Self::aligned(HAlign::Right, left, width, baseline)
    }
}

fn blend(under: PremultipliedColorU8, color: Color, coverage: f32) -> PremultipliedColorU8 {
    let covered = coverage * color.alpha();
    let mix = |channel: u8, added: f32| {
        (f32::from(channel) * (1.0 - covered) + added * 255.0 * covered).round() as u8
    };

    PremultipliedColorU8::from_rgba(
        mix(under.red(), color.red()),
        mix(under.green(), color.green()),
        mix(under.blue(), color.blue()),
        mix(under.alpha(), 1.0),
    )
    .expect("a blended pixel stays a valid premultiplied color")
}

/// The highest of the 16 coverage levels a mask keeps.
const LEVELS: u8 = 15;

/// `coverage` quantized to one of 16 levels.
fn four_bits(coverage: f32) -> u8 {
    (coverage.clamp(0.0, 1.0) * f32::from(LEVELS)).round() as u8
}

/// Start positions per pixel, across and down; a glyph is rasterized once per position.
const SUBPIXELS: f32 = 4.0;

/// A rasterized glyph: `width` by `height` coverage levels up to [`LEVELS`], row by row, from
/// `(left, top)`.
pub struct GlyphMask {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    coverage: Vec<u8>,
}

/// Face, glyph, size bits, subpixel offset.
pub type GlyphKey = (Face, GlyphId, u32, (u8, u8));

/// `None` for a glyph that draws nothing, such as a space; kept in `glyphs`.
fn mask(
    glyphs: &mut HashMap<GlyphKey, Option<GlyphMask>>,
    face: Face,
    glyph: GlyphId,
    scale: PxScale,
    offset: (u8, u8),
) -> Option<&GlyphMask> {
    glyphs
        .entry((face, glyph, scale.y.to_bits(), offset))
        .or_insert_with(|| {
            let position = point(
                f32::from(offset.0) / SUBPIXELS,
                f32::from(offset.1) / SUBPIXELS,
            );
            let outlined = face
                .font()
                .outline_glyph(glyph.with_scale_and_position(scale, position))?;
            let bounds = outlined.px_bounds();
            let (width, height) = (bounds.width() as i32, bounds.height() as i32);

            if width <= 0 || height <= 0 {
                return None;
            }

            let mut coverage = vec![0; (width * height) as usize];

            outlined.draw(|x, y, covered| {
                coverage[(y * width as u32 + x) as usize] = four_bits(covered);
            });

            Some(GlyphMask {
                left: bounds.min.x as i32,
                top: bounds.min.y as i32,
                width,
                height,
                coverage,
            })
        })
        .as_ref()
}

/// Blends `mask` in `color` into the `stride` pixels wide `pixels`, its top left corner at
/// `(left, top)`, only within the pixel `columns` and `rows`.
fn paint(
    pixels: &mut [PremultipliedColorU8],
    stride: i32,
    mask: &GlyphMask,
    (left, top): (i32, i32),
    (columns, rows): (&Range<i32>, &Range<i32>),
    color: Color,
) {
    let across = left.max(columns.start)..(left + mask.width).min(columns.end);
    let down = top.max(rows.start)..(top + mask.height).min(rows.end);

    if across.is_empty() {
        return;
    }

    for y in down {
        let mask_row = (y - top) * mask.width - left;
        let canvas_row = y * stride;
        let coverage =
            &mask.coverage[(mask_row + across.start) as usize..(mask_row + across.end) as usize];
        let targets =
            &mut pixels[(canvas_row + across.start) as usize..(canvas_row + across.end) as usize];

        for (pixel, &covered) in targets.iter_mut().zip(coverage) {
            if covered > 0 {
                *pixel = blend(*pixel, color, f32::from(covered) / f32::from(LEVELS));
            }
        }
    }
}

fn split(position: f32) -> (i32, u8) {
    let pixel = position.floor();

    (
        pixel as i32,
        ((position - pixel) * SUBPIXELS)
            .floor()
            .min(SUBPIXELS - 1.0) as u8,
    )
}

/// Glyphs advance by whole screen pixels: `advance` at `pixels` device pixels per screen pixel.
fn whole(advance: f32, pixels: f32) -> f32 {
    (advance / pixels).round() * pixels
}

fn width(face: Face, scale: PxScale, text: &str, pixels: f32) -> f32 {
    let scaled = face.font().as_scaled(scale);

    text.chars()
        .map(|ch| whole(scaled.h_advance(scaled.glyph_id(ch)), pixels))
        .sum()
}

/// Draws `text` at an em of `size` screen px; glyphs are rasterized at the scale of `canvas`.
pub fn draw(
    canvas: &mut Canvas<'_>,
    face: Face,
    text: &str,
    size: f32,
    color: Color,
    at: Placement,
) {
    draw_within(canvas, face, text, size, color, at, (f32::MIN, f32::MAX));
}

/// Like [`draw`], but only the part between the screen x positions `within` shows.
pub fn draw_within(
    canvas: &mut Canvas<'_>,
    face: Face,
    text: &str,
    size: f32,
    color: Color,
    at: Placement,
    within: (f32, f32),
) {
    let density = canvas.scale();
    let (origin_x, origin_y) = canvas.origin();
    let within = (within.0 * density, within.1 * density);
    let size = size * density;
    let at = Placement {
        left: at.left * density,
        width: at.width * density,
        baseline: at.baseline * density,
        ..at
    };
    let scale = em(face, size);
    let scaled = face.font().as_scaled(scale);
    let start_x = whole(
        match at.align {
            HAlign::Left => at.left,
            HAlign::Mid => at.left + (at.width - width(face, scale, text, density)) / 2.0,
            HAlign::Right => at.left + at.width - width(face, scale, text, density),
        },
        density,
    );
    let (width, height) = canvas.size();
    let stride = width.cast_signed();
    let column_at = |x: f32| (x.ceil() - origin_x as f32).clamp(0.0, stride as f32) as i32;
    let columns = column_at(within.0)..column_at(within.1);
    let rows = 0..height.cast_signed();
    let (pixmap, cache) = canvas.parts();
    let pixels = pixmap.pixels_mut();
    let glyphs = &mut cache.glyphs;
    let (row, offset_y) = split(at.baseline);

    text.chars().fold(start_x, |cursor, ch| {
        let glyph = scaled.glyph_id(ch);
        let (column, offset_x) = split(cursor);

        if let Some(mask) = mask(glyphs, face, glyph, scale, (offset_x, offset_y)) {
            let corner = (column + mask.left - origin_x, row + mask.top - origin_y);

            paint(pixels, stride, mask, corner, (&columns, &rows), color);
        }

        cursor + whole(scaled.h_advance(glyph), density)
    });
}
