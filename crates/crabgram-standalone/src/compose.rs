//! The window's picture: the strip's bar, the screen below it, the drop-down over both.

use tiny_skia::{Pixmap, PixmapMut};

use crate::{
    CONTENT,
    render::{self, Renderer, Scene},
    strip::Strip,
};

/// Composes the picture on a canvas it keeps between pictures: the renderer draws the screen
/// straight into it, under the strip's bar as it last drew it.
#[derive(Default)]
pub struct Composer {
    renderer: Renderer,
    canvas: Option<Pixmap>,
    bar: Option<Pixmap>,
    /// The revision of the strip and the size of the bar last drawn.
    bar_drawn: Option<(u64, (u32, u32))>,
    /// Whether the last picture had a list over the bar.
    listed: bool,
}

impl Composer {
    /// `strip` over the screen of `scene`, at `scale` times their size.
    pub fn compose(&mut self, strip: &Strip, scene: Scene, scale: f32) -> Option<&Pixmap> {
        let (width, screen_height) = render::screen_size(scale);
        let height = (CONTENT.1 as f32 * scale).round() as u32;
        let screen_top = height.saturating_sub(screen_height);
        let bar_size = (width, screen_top);
        let bar_key = (strip.revision(), bar_size);

        let redrawn = self.bar_drawn != Some(bar_key);

        if redrawn {
            let bar = render::resized(&mut self.bar, bar_size)?;

            strip.draw_bar(&mut self.renderer.canvas(bar.as_mut(), scale));
            self.bar_drawn = Some(bar_key);
        }

        let canvas = render::resized(&mut self.canvas, (width, screen_top + screen_height))?;

        if let Some(bar) = self.bar.as_ref().filter(|_| redrawn || self.listed) {
            render::blit(&mut canvas.as_mut(), bar, (0, 0));
        }

        let below_bar = screen_top as usize * width as usize * 4;
        let screen =
            PixmapMut::from_bytes(&mut canvas.data_mut()[below_bar..], width, screen_height)?;

        self.renderer.render(screen, scene, scale);

        self.listed = strip.is_listing();

        if self.listed {
            strip.draw_overlay(&mut self.renderer.canvas(canvas.as_mut(), scale));
        }

        Some(canvas)
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use crabgram_styling::{BlockSettings, host, ui::Model};

    use super::*;
    use crate::{
        render::{Images, State},
        strip::{self, ContentPoint},
    };

    fn model() -> Model {
        Model::new(
            "urn:crabgram:test",
            "Probe",
            NonZeroU32::new(2).unwrap(),
            Vec::new(),
            BlockSettings::default(),
        )
    }

    /// The picture `composer` makes of `strip` over an empty screen.
    fn picture(composer: &mut Composer, strip: &Strip, scale: f32) -> Option<Vec<u8>> {
        let model = model();
        let (state, images) = (State::new(&model, None), Images::new());

        composer
            .compose(strip, Scene::new(&model, &state, &images), scale)
            .map(|it| it.data().to_vec())
    }

    #[test]
    fn composing_again_settles_on_one_picture() {
        let mut strip = Strip::new(2);

        strip.set_status("plugin 48000 Hz");

        for scale in [1.0, 1.13, 1.37, 2.0] {
            let mut composer = Composer::default();

            (0..16).for_each(|_| {
                picture(&mut composer, &strip, scale);
            });
            assert_eq!(
                picture(&mut composer, &strip, scale),
                picture(&mut composer, &strip, scale),
                "the bar at {scale} settles"
            );
        }
    }

    #[test]
    fn the_canvas_is_as_high_as_the_window() {
        let strip = Strip::new(2);
        let scale = 1.13;
        let device = |pixels: u32| (pixels as f32 * scale).round() as u32;
        let window = device(host::SCREEN.1 + u32::from(strip::HEIGHT));
        let model = model();
        let (state, images) = (State::new(&model, None), Images::new());

        assert_ne!(
            device(u32::from(strip::HEIGHT)) + device(host::SCREEN.1),
            window
        );
        assert_eq!(
            Composer::default()
                .compose(&strip, Scene::new(&model, &state, &images), scale)
                .map(Pixmap::height),
            Some(window)
        );
    }

    #[test]
    fn every_pixel_is_covered_at_any_scale() {
        let strip = Strip::new(2);

        for scale in [1.0, 1.07, 1.13, 1.25, 1.37, 1.5, 1.75, 2.0] {
            let picture = picture(&mut Composer::default(), &strip, scale).unwrap();

            assert!(
                picture
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| pixel[3] == u8::MAX),
                "a row shows through at {scale}"
            );
        }
    }

    #[test]
    fn a_new_status_and_a_closed_list_show_as_on_a_fresh_canvas() {
        let mut strip = Strip::new(2);
        let mut composer = Composer::default();
        let scale = 1.13;
        let fresh = |strip: &Strip| picture(&mut Composer::default(), strip, scale);

        picture(&mut composer, &strip, scale);
        strip.set_status("audio failed");
        assert_eq!(picture(&mut composer, &strip, scale), fresh(&strip));

        assert_eq!(strip.press(ContentPoint::new(100.0, 16.0)), None);
        assert!(strip.is_listing());
        assert_eq!(picture(&mut composer, &strip, scale), fresh(&strip));

        assert_eq!(strip.press(ContentPoint::new(5_000.0, 5_000.0)), None);
        assert_eq!(picture(&mut composer, &strip, scale), fresh(&strip));
    }
}
