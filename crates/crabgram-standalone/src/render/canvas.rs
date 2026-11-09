//! What the preview draws on: device pixels at the scale of the drawings it keeps.

use crabgram_styling::host;
use tiny_skia::{PixmapMut, Transform};

use super::cache::Cache;

/// The scale nearest `scale` at which the screen is a whole number of device pixels wide, the
/// scale everything is drawn at.
#[must_use]
pub fn snapped(scale: f32) -> f32 {
    let width = host::SCREEN.0 as f32;

    (width * scale).round() / width
}

/// Device pixels showing the part of a picture from device pixel `origin` on, drawn at the scale
/// of `cache`, which keeps what they draw.
pub struct Canvas<'a> {
    pixmap: PixmapMut<'a>,
    origin: (i32, i32),
    cache: &'a mut Cache,
}

impl<'a> Canvas<'a> {
    /// `pixmap` showing a picture from its top left corner on.
    #[must_use]
    pub const fn new(pixmap: PixmapMut<'a>, cache: &'a mut Cache) -> Self {
        Self::part(pixmap, (0, 0), cache)
    }

    /// `pixmap` showing a picture from its device pixel `origin` on.
    #[must_use]
    pub const fn part(pixmap: PixmapMut<'a>, origin: (i32, i32), cache: &'a mut Cache) -> Self {
        Self {
            pixmap,
            origin,
            cache,
        }
    }

    pub const fn pixmap(&mut self) -> &mut PixmapMut<'a> {
        &mut self.pixmap
    }

    pub const fn cache(&mut self) -> &mut Cache {
        self.cache
    }

    /// The pixmap and the cache at once.
    pub const fn parts(&mut self) -> (&mut PixmapMut<'a>, &mut Cache) {
        (&mut self.pixmap, self.cache)
    }

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.pixmap.width(), self.pixmap.height())
    }

    #[must_use]
    pub const fn scale(&self) -> f32 {
        self.cache.scale()
    }

    /// The device pixel of the picture at the top left corner.
    #[must_use]
    pub const fn origin(&self) -> (i32, i32) {
        self.origin
    }

    /// Draws screen positions where they show.
    #[must_use]
    pub fn transform(&self) -> Transform {
        let (x, y) = self.origin;

        Transform::from_scale(self.scale(), self.scale()).post_translate(-x as f32, -y as f32)
    }

    /// The device pixel of the picture nearest the screen position `(x, y)`.
    #[must_use]
    pub fn device_pixel(&self, (x, y): (f32, f32)) -> (i32, i32) {
        (
            (x * self.scale()).round() as i32,
            (y * self.scale()).round() as i32,
        )
    }

    /// Where device pixel `(x, y)` of the picture is on the pixmap.
    #[must_use]
    pub const fn on_pixmap(&self, (x, y): (i32, i32)) -> (i32, i32) {
        (x - self.origin.0, y - self.origin.1)
    }
}
