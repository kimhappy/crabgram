//! What the preview keeps between pictures, all at one scale.

use std::collections::HashMap;

use crabgram_styling::Framing;
use tiny_skia::Pixmap;

use super::{
    canvas::snapped,
    chrome::Art,
    renderer::screen_size,
    stock::BackgroundSpot,
    text::{GlyphKey, GlyphMask},
};

/// The drawings the preview made at one screen size, forgotten together when it changes.
pub struct Cache {
    scale: f32,
    /// The screen in device pixels: two scales that round to the same size draw the same.
    size: (u32, u32),
    /// Styling image frames by path, stretched to the scale.
    pub(super) frames: HashMap<String, HashMap<(Framing, u32), Pixmap>>,
    /// The rasterized fixed drawings.
    pub(super) art: HashMap<Art, Pixmap>,
    /// `None` for a glyph that draws nothing.
    pub(super) glyphs: HashMap<GlyphKey, Option<GlyphMask>>,
    /// Stock widget backgrounds by path, clipped to the card at each placement.
    pub(super) clipped: HashMap<String, HashMap<BackgroundSpot, Pixmap>>,
    /// The layer a dimmed widget is drawn on, as large as the last one reached.
    pub(super) dimmed: Option<Pixmap>,
    /// The text of the value last drawn, kept for its allocation.
    pub(super) number: String,
}

impl Cache {
    #[must_use]
    pub fn new(scale: f32) -> Self {
        Self {
            scale: snapped(scale),
            size: screen_size(scale),
            frames: HashMap::new(),
            art: HashMap::new(),
            glyphs: HashMap::new(),
            clipped: HashMap::new(),
            dimmed: None,
            number: String::new(),
        }
    }

    /// Device pixels per screen pixel.
    #[must_use]
    pub const fn scale(&self) -> f32 {
        self.scale
    }

    /// Draws at `scale` from now on, forgetting every drawing if the screen is another size at
    /// it; whether it is.
    pub fn rescale(&mut self, scale: f32) -> bool {
        let other = screen_size(scale) != self.size;

        if other {
            *self = Self::new(scale);
        }

        other
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new(1.0)
    }
}
