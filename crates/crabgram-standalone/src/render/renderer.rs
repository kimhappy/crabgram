//! Draws a page of the preview: the styling's images over the built-in widgets.

use std::{collections::HashMap, path::Path};

use crabgram::ControlState;
use crabgram_styling::{
    Alignment, BlockSettings, Button, Bypass, FrameCount, Framing, Image, Pagination, Slot,
    Styling, Widget, host, ui::Model,
};
use tiny_skia::{FilterQuality, IntRect, Pixmap, PixmapMut, PixmapPaint, Transform};

use super::{
    cache::Cache,
    canvas::{Canvas, snapped},
    look::{self, Area, Ring},
    state::{Focus, Overlay, State},
    stock,
};

/// The styling's images by path.
pub type Images = HashMap<String, Pixmap>;

/// What a picture is drawn from: the model, what the screen shows and the styling's images.
#[derive(Clone, Copy)]
pub struct Scene<'a> {
    model: &'a Model,
    state: &'a State,
    images: &'a Images,
}

impl<'a> Scene<'a> {
    #[must_use]
    pub const fn new(model: &'a Model, state: &'a State, images: &'a Images) -> Self {
        Self {
            model,
            state,
            images,
        }
    }

    #[must_use]
    pub const fn model(self) -> &'a Model {
        self.model
    }

    #[must_use]
    pub const fn state(self) -> &'a State {
        self.state
    }

    #[must_use]
    pub const fn images(self) -> &'a Images {
        self.images
    }
}

/// The images the styling of `model` names, from `bundle`; the ones it cannot read left out, each
/// with a warning.
#[must_use]
pub fn load_images(bundle: &Path, model: &Model) -> Images {
    let styling = Styling::default().with_block_settings(model.settings().clone());

    styling
        .images(model.controls())
        .into_keys()
        .filter_map(|path| {
            let file = bundle.join(path);

            Pixmap::load_png(&file)
                .inspect_err(|error| {
                    eprintln!("styling: cannot load {}: {error}", file.display());
                })
                .ok()
                .map(|image| (path.to_owned(), image))
        })
        .collect()
}

fn size(image: &Pixmap) -> (u32, u32) {
    (image.width(), image.height())
}

pub(super) fn blank((width, height): (u32, u32)) -> Pixmap {
    Pixmap::new(width, height).expect("a frame has a positive size")
}

/// `slot`'s pixmap if it is `size` large, else a new blank one; `None` for an empty size.
pub fn resized(slot: &mut Option<Pixmap>, size: (u32, u32)) -> Option<&mut Pixmap> {
    if slot.as_ref().is_none_or(|it| self::size(it) != size) {
        *slot = Pixmap::new(size.0, size.1);
    }

    slot.as_mut()
}

pub(super) fn paste(pixmap: &mut PixmapMut<'_>, image: &Pixmap, (x, y): (i32, i32)) {
    pixmap.draw_pixmap(
        x,
        y,
        image.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

/// Copies `src` onto `dst` at `(x, y)`, clipped, replacing what is there. Far cheaper than
/// [`paste`] when `src` is opaque.
pub fn blit(dst: &mut PixmapMut<'_>, src: &Pixmap, (x, y): (i32, i32)) {
    let (dst_width, dst_height) = (dst.width().cast_signed(), dst.height().cast_signed());
    let (src_width, src_height) = (src.width().cast_signed(), src.height().cast_signed());
    let (left, right) = (x.max(0), (x + src_width).min(dst_width));
    let (top, bottom) = (y.max(0), (y + src_height).min(dst_height));

    if left >= right || top >= bottom {
        return;
    }

    let row_bytes = (right - left) as usize * 4;
    let source = src.data();
    let target = dst.data_mut();

    for row in top..bottom {
        let from = ((row - y) * src_width + (left - x)) as usize * 4;
        let to = (row * dst_width + left) as usize * 4;

        target[to..to + row_bytes].copy_from_slice(&source[from..from + row_bytes]);
    }
}

fn frame(image: &Pixmap, framing: Framing, index: u32) -> Pixmap {
    let cut = framing.cut(size(image));
    let (x, y) = cut.offset(index);
    let mut frame = blank(cut.frame());

    paste(
        &mut frame.as_mut(),
        image,
        (-x.cast_signed(), -y.cast_signed()),
    );
    frame
}

/// Frame `index` of `image` stretched to the scale of `cache`, kept there by `path`.
pub(super) fn scaled_frame<'a>(
    cache: &'a mut Cache,
    image: &Pixmap,
    path: &str,
    framing: Framing,
    index: u32,
) -> &'a Pixmap {
    let scale = cache.scale();

    cache
        .frames
        .entry(path.to_owned())
        .or_default()
        .entry((framing, index))
        .or_insert_with(|| {
            let cut = frame(image, framing, index);

            #[expect(clippy::float_cmp, reason = "only an exact 1 needs no stretching")]
            let unscaled = scale == 1.0;

            if unscaled {
                return cut;
            }

            let mut stretched = blank((
                ((cut.width() as f32 * scale).round() as u32).max(1),
                ((cut.height() as f32 * scale).round() as u32).max(1),
            ));
            let (across, down) = (
                stretched.width() as f32 / cut.width() as f32,
                stretched.height() as f32 / cut.height() as f32,
            );

            stretched.draw_pixmap(
                0,
                0,
                cut.as_ref(),
                &PixmapPaint {
                    quality: FilterQuality::Bicubic,
                    ..PixmapPaint::default()
                },
                Transform::from_scale(across, down),
                None,
            );
            stretched
        })
}

/// Draws frame `index` of the image at `path`, cut by `framing`, with its corner at `place`.
pub(super) fn draw(
    canvas: &mut Canvas<'_>,
    images: &Images,
    path: &str,
    framing: Framing,
    index: u32,
    place: (i32, i32),
) {
    let Some(image) = images.get(path) else {
        return;
    };
    let at = canvas.on_pixmap(canvas.device_pixel(look::point(place)));
    let (pixmap, cache) = canvas.parts();
    let frame = scaled_frame(cache, image, path, framing, index);

    paste(pixmap, frame, at);
}

fn aligned(image: &Image, (width, height): (u32, u32), default: Alignment) -> (i32, i32) {
    let (screen_width, screen_height) = host::SCREEN;
    let free_x = screen_width.cast_signed() - width.cast_signed();
    let free_y = screen_height.cast_signed() - height.cast_signed();
    let (x, y) = match image.alignment().unwrap_or(default) {
        Alignment::TopLeft => (0, 0),
        Alignment::TopMid => (free_x / 2, 0),
        Alignment::TopRight => (free_x, 0),
        Alignment::LeftMid => (0, free_y / 2),
        Alignment::Center => (free_x / 2, free_y / 2),
        Alignment::RightMid => (free_x, free_y / 2),
        Alignment::BottomLeft => (0, free_y),
        Alignment::BottomMid => (free_x / 2, free_y),
        Alignment::BottomRight => (free_x, free_y),
    };

    let (offset_x, offset_y) = image.offset();

    (x + offset_x, y + offset_y)
}

fn draw_top_bar(canvas: &mut Canvas<'_>, scene: Scene) {
    let (model, state, images) = (scene.model, scene.state, scene.images);
    let bypassed = state.bypassed();
    let settings = model.settings();
    let bar = settings.top_bar();
    let bypass = settings.bypass();

    if bar.is_none_or(|it| it.background().is_none()) {
        look::top_bar(canvas, model.name(), bypassed, state.active_scene_only());
    }

    [
        (bar.and_then(|it| it.background()), (0, 0)),
        (
            bar.and_then(|it| it.block_name()),
            host::position::BLOCK_NAME,
        ),
        (
            bypass.and_then(|it| it.background()),
            host::position::BYPASS,
        ),
        (
            bar.and_then(|it| it.scene_control()).map(|it| {
                if state.active_scene_only() {
                    it.active_scene()
                } else {
                    it.all_scenes()
                }
            }),
            host::position::SCENE_CONTROL,
        ),
    ]
    .into_iter()
    .filter_map(|(image, place)| Some((image?, place)))
    .for_each(|(image, place)| draw(canvas, images, image.path(), Framing::Free, 0, place));

    if let Some((path, framing)) = bypass.and_then(Bypass::switch_framing) {
        // Frames are on, then off (see `Bypass::control`).
        draw(
            canvas,
            images,
            path,
            framing,
            u32::from(bypassed),
            host::position::BYPASS,
        );
    }

    [
        (Button::Close, host::position::CLOSE),
        (Button::Remove, host::position::BUTTONS[0]),
        (Button::Swap, host::position::BUTTONS[1]),
        (Button::More, host::position::BUTTONS[2]),
    ]
    .into_iter()
    .filter_map(|(button, place)| Some((bar?.buttons().get(&button)?, button, place)))
    .for_each(|(path, button, place)| draw(canvas, images, path, button.framing(), 0, place));

    if state.focus() == Some(Focus::Bypass) {
        let ring = if bypass.and_then(Bypass::switch_framing).is_some() {
            Ring::StyledSwitch
        } else {
            Ring::Switch
        };

        look::ring(canvas, Area::at(host::position::BYPASS, host::BYPASS), ring);
    }
}

/// How far left of its slot every widget sits.
const SHIFT: f32 = 1.0;

/// The top left corner of the widget of `slot`, where it is drawn and touched.
pub(super) fn origin(slot: &Slot) -> (f32, f32) {
    (slot.x() as f32 - SHIFT, host::WIDGET_TOP as f32)
}

fn draw_widget(canvas: &mut Canvas<'_>, scene: Scene, slot: &Slot) {
    let place = origin(slot);

    match scene.model.settings().widget_of(slot.control()) {
        Some(widget) => draw_styled(canvas, scene, widget, slot, place),
        None => stock::draw_widget(canvas, scene, slot, place),
    }
}

/// The widget of `slot` in the styling: its background, the overlay of its state and its control
/// frame.
fn draw_styled(
    canvas: &mut Canvas<'_>,
    scene: Scene,
    widget: &Widget,
    slot: &Slot,
    place: (f32, f32),
) {
    let (state, images) = (scene.state, scene.images);
    let (x, y) = (place.0 as i32, place.1 as i32);

    if let Some(background) = widget.background() {
        draw(canvas, images, background.path(), Framing::Free, 0, (x, y));
    }

    let overlay = match state.states()[slot.position()] {
        ControlState::Inactive => widget.overlays().inactive(),
        ControlState::Blocked => widget.overlays().blocked(),
        ControlState::Normal => None,
    };

    if let Some(overlay) = overlay {
        draw(canvas, images, overlay, Framing::Free, 0, (x, y));
    }

    let image = widget.control();

    if let Some(file) = images.get(image.path()) {
        let framing = Framing::control(FrameCount::AtLeast(2));
        let cut = framing.cut(size(file));
        let (width, height) = cut.frame();
        let (widget_width, widget_height) = host::WIDGET;
        let place = (
            x + (widget_width.cast_signed() - width.cast_signed()) / 2,
            y + (widget_height.cast_signed() - height.cast_signed()) / 2,
        );
        let normalized = slot.control().normalized(state.values()[slot.position()]);
        let index = (normalized * cut.frames().saturating_sub(1) as f32).round() as u32;

        draw(canvas, images, image.path(), framing, index, place);
    }
}

/// The styled pagination dots, if the styling has them rather than a button.
const fn dots(settings: &BlockSettings) -> Option<&Image> {
    match settings.pagination() {
        Some(Pagination::Dots(image)) => Some(image),
        Some(Pagination::Button(_)) | None => None,
    }
}

/// Where the styled pagination dots sit, if the styling has them.
#[must_use]
pub fn styled_dots(model: &Model, images: &Images) -> Option<Area> {
    let image = dots(model.settings())?;
    let pages = model.settings().pages(model.controls());
    let frame = Framing::row(pages)
        .cut(size(images.get(image.path())?))
        .frame();
    let (x, y) = aligned(image, frame, Alignment::BottomMid);

    Some(Area::at((x - look::DOTS_SHIFT as i32, y), frame))
}

fn draw_dots(canvas: &mut Canvas<'_>, scene: Scene, page: u32) {
    let (model, state, images) = (scene.model, scene.state, scene.images);

    match (dots(model.settings()), state.dots()) {
        (Some(image), Some(area)) => draw(
            canvas,
            images,
            image.path(),
            Framing::row(state.pages()),
            page,
            (area.x() as i32, area.y() as i32),
        ),
        _ => look::dots(canvas, state.pages(), page),
    }
}

/// Inputs change only with the screen; outputs are animated by the plugin and draw over them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Layer {
    Inputs,
    Outputs,
}

impl Layer {
    fn holds(self, slot: &Slot) -> bool {
        slot.control().is_output() == (self == Self::Outputs)
    }
}

/// The widgets of `page` in `layer`, plus the background for the inputs.
fn draw_content(canvas: &mut Canvas<'_>, scene: Scene, page: u32, layer: Layer) {
    let (model, state, images) = (scene.model, scene.state, scene.images);

    if layer == Layer::Inputs {
        look::clear(canvas);

        if let Some(background) = model.settings().background() {
            draw(
                canvas,
                images,
                background.path(),
                Framing::pages(state.pages()),
                page,
                (0, 0),
            );
        }
    }

    state
        .slots()
        .iter()
        .filter(|slot| slot.page() == page && layer.holds(slot))
        .for_each(|slot| {
            draw_widget(canvas, scene, slot);

            if state.focus() == Some(Focus::Widget(slot.position())) {
                let (x, y) = origin(slot);
                let area = Area::new(
                    x,
                    y,
                    host::span_width(slot.slots()) as f32,
                    host::WIDGET.1 as f32,
                );

                look::ring(canvas, area, Ring::Widget);
            }
        });
}

/// What stays put while the pages scroll.
fn draw_chrome(canvas: &mut Canvas<'_>, scene: Scene, page: u32) {
    draw_top_bar(canvas, scene);
    draw_dots(canvas, scene, page);
    look::edge_meters(canvas);
}

/// Where the about page puts the block name image.
const ABOUT_NAME_X: i32 = (host::SCREEN.0 as i32 - host::WIDGET.0 as i32) / 2;

/// The about page under the top bar.
fn draw_about(canvas: &mut Canvas<'_>, scene: Scene) {
    let (model, images) = (scene.model, scene.images);
    let settings = model.settings();
    let bar = settings.top_bar();
    let name = bar.and_then(|it| it.block_name());

    look::clear(canvas);

    if let Some(about) = settings.about() {
        draw(
            canvas,
            images,
            about.path(),
            Framing::Free,
            0,
            (0, host::TOP_BAR.1 as i32),
        );
    }

    if let Some(background) = bar.and_then(|it| it.background()) {
        draw(canvas, images, background.path(), Framing::Free, 0, (0, 0));
    } else {
        look::back_bar(canvas, if name.is_some() { "" } else { model.name() });
    }

    if let Some(path) = bar.and_then(|it| it.buttons().get(&Button::Back)) {
        draw(
            canvas,
            images,
            path,
            Button::Back.framing(),
            0,
            host::position::CLOSE,
        );
    }

    if let Some(name) = name {
        draw(
            canvas,
            images,
            name.path(),
            Framing::Free,
            0,
            (ABOUT_NAME_X, host::position::BLOCK_NAME.1),
        );
    }
}

/// Everything the input layer of `page` depends on: each input's value and state.
fn input_values(state: &State, page: u32) -> impl Iterator<Item = f32> {
    state
        .slots()
        .iter()
        .filter(move |slot| slot.page() == page && Layer::Inputs.holds(slot))
        .flat_map(|slot| {
            [
                state.values()[slot.position()],
                f32::from(state.states()[slot.position()] as u8),
                f32::from(state.focus() == Some(Focus::Widget(slot.position()))),
            ]
        })
}

/// The screen's size in device pixels at `scale` times its size, as drawn: at the
/// [`snapped`] scale.
#[must_use]
pub fn screen_size(scale: f32) -> (u32, u32) {
    let (width, height) = host::SCREEN;
    let snapped = snapped(scale);

    (
        (width as f32 * snapped).round() as u32,
        (height as f32 * snapped).round() as u32,
    )
}

/// The input layer of every page drawn, by page: it redraws only when an input value changed, and
/// output widgets draw over a copy of it.
#[derive(Default)]
struct Pages {
    inputs: HashMap<u32, (Vec<f32>, Pixmap)>,
    layered: Option<Pixmap>,
}

impl Pages {
    /// Blits `page` onto `screen`, shifted left by `shift` device pixels.
    fn draw(&mut self, screen: &mut Canvas<'_>, scene: Scene, page: u32, shift: i32) {
        let state = scene.state;
        let outdated = self
            .inputs
            .get(&page)
            .is_none_or(|(seen, _)| !seen.iter().copied().eq(input_values(state, page)));

        if outdated {
            let (seen, inputs) = self
                .inputs
                .entry(page)
                .or_insert_with(|| (Vec::new(), blank(screen.size())));

            draw_content(
                &mut Canvas::new(inputs.as_mut(), screen.cache()),
                scene,
                page,
                Layer::Inputs,
            );
            seen.clear();
            seen.extend(input_values(state, page));
        }

        let inputs = &self.inputs[&page].1;
        let shows_outputs = state
            .slots()
            .iter()
            .any(|slot| slot.page() == page && Layer::Outputs.holds(slot));

        if shift == 0 {
            blit(screen.pixmap(), inputs, (0, 0));

            if shows_outputs {
                draw_content(screen, scene, page, Layer::Outputs);
            }
        } else if shows_outputs {
            let layered =
                resized(&mut self.layered, screen.size()).expect("the screen has a positive size");

            layered.data_mut().copy_from_slice(inputs.data());
            draw_content(
                &mut Canvas::new(layered.as_mut(), screen.cache()),
                scene,
                page,
                Layer::Outputs,
            );
            blit(screen.pixmap(), layered, (-shift, 0));
        } else {
            blit(screen.pixmap(), inputs, (-shift, 0));
        }
    }
}

/// Everything the chrome depends on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChromeKey {
    bypassed: bool,
    bypass_focused: bool,
    active_scene_only: bool,
    page: u32,
}

/// Columns of nothing that still join two pieces of a band, in device pixels.
const JOINED_GAP: usize = 32;

/// The ranges of `0..length` where `drawn` holds, ranges closer than [`JOINED_GAP`] joined.
pub(super) fn runs(length: usize, drawn: impl Fn(usize) -> bool) -> Vec<(usize, usize)> {
    (0..length).filter(|&index| drawn(index)).fold(
        Vec::new(),
        |mut runs: Vec<(usize, usize)>, index| {
            match runs.last_mut() {
                Some((_, end)) if index <= *end + JOINED_GAP => *end = index + 1,
                _ => runs.push((index, index + 1)),
            }
            runs
        },
    )
}

/// A part of a layer, cut out.
struct Piece {
    /// Its top left corner in the layer.
    place: (i32, i32),
    pixmap: Pixmap,
    /// Whether every pixel covers what is under it, so that it is copied rather than blended.
    opaque: bool,
}

impl Piece {
    fn new(place: (i32, i32), pixmap: Pixmap) -> Self {
        let opaque = pixmap.pixels().iter().all(|it| it.alpha() == u8::MAX);

        Self {
            place,
            pixmap,
            opaque,
        }
    }

    fn draw(&self, screen: &mut PixmapMut<'_>) {
        if self.opaque {
            blit(screen, &self.pixmap, self.place);
        } else {
            paste(screen, &self.pixmap, self.place);
        }
    }
}

/// The parts of `layer` with anything drawn: bands of rows, each split into runs of columns.
fn pieces(layer: &Pixmap) -> Vec<Piece> {
    let width = layer.width() as usize;
    let alpha = |x: usize, y: usize| layer.pixels()[y * width + x].alpha() > 0;

    runs(layer.height() as usize, |y| (0..width).any(|x| alpha(x, y)))
        .into_iter()
        .flat_map(|(top, bottom)| {
            runs(width, |x| (top..bottom).any(|y| alpha(x, y)))
                .into_iter()
                .map(move |(left, right)| (left, top, right, bottom))
        })
        .filter_map(|(left, top, right, bottom)| {
            let area = IntRect::from_ltrb(left as i32, top as i32, right as i32, bottom as i32)?;

            Some(Piece::new((area.x(), area.y()), layer.clone_rect(area)?))
        })
        .collect()
}

/// What stays put while the pages scroll, redrawn only when what it shows changed. Kept as the
/// pieces with anything drawn, so that the screen blends only those.
#[derive(Default)]
struct Chrome(Option<(ChromeKey, Vec<Piece>)>);

impl Chrome {
    /// The pieces of the chrome of `page` on `screen`, with their place.
    fn pieces(&mut self, scene: Scene, page: u32, screen: &mut Canvas<'_>) -> &[Piece] {
        let state = scene.state;
        let key = ChromeKey {
            bypassed: state.bypassed(),
            bypass_focused: state.focus() == Some(Focus::Bypass),
            active_scene_only: state.active_scene_only(),
            page,
        };

        match &mut self.0 {
            Some((seen, pieces)) if *seen == key => pieces,
            other => {
                let mut layer = blank(screen.size());

                draw_chrome(
                    &mut Canvas::new(layer.as_mut(), screen.cache()),
                    scene,
                    page,
                );
                &other.insert((key, pieces(&layer))).1
            }
        }
    }
}

/// The pages and the chrome the screen is made of.
#[derive(Default)]
struct Layers {
    pages: Pages,
    chrome: Chrome,
}

impl Layers {
    /// Draws the page the screen is on, or the two it is between, then the chrome over them.
    fn draw(&mut self, screen: &mut Canvas<'_>, scene: Scene) {
        let state = scene.state;
        let position = state.position().clamp(0.0, (state.pages() - 1) as f32);
        let lower = position.floor() as u32;
        let width = screen.size().0.cast_signed();
        let shift = ((position - lower as f32) * width as f32).round() as i32;

        self.pages.draw(screen, scene, lower, shift);

        if shift != 0 {
            self.pages.draw(screen, scene, lower + 1, shift - width);
        }

        for piece in self.chrome.pieces(scene, position.round() as u32, screen) {
            piece.draw(screen.pixmap());
        }
    }
}

/// Renders the screen, keeping each page's input layer and the chrome between pictures.
///
/// Serves one model and one set of images at a time.
#[derive(Default)]
pub struct Renderer {
    layers: Layers,
    cache: Cache,
}

impl Renderer {
    /// Draws at `scale` times the screen's size from now on, forgetting every drawing of another.
    fn rescale(&mut self, scale: f32) {
        if self.cache.rescale(scale) {
            self.layers = Layers::default();
        }
    }

    /// A canvas of `pixmap` at `scale` times the screen's size, for what is drawn beside the
    /// screen; it keeps its drawings with the screen's.
    pub fn canvas<'a>(&'a mut self, pixmap: PixmapMut<'a>, scale: f32) -> Canvas<'a> {
        self.rescale(scale);
        Canvas::new(pixmap, &mut self.cache)
    }

    /// Renders the current page over all of `screen`, which is [`screen_size`] at `scale`.
    pub fn render(&mut self, screen: PixmapMut<'_>, scene: Scene, scale: f32) {
        debug_assert_eq!(
            (screen.width(), screen.height()),
            screen_size(scale),
            "the screen is as large as the scale makes it"
        );

        self.rescale(scale);

        let mut screen = Canvas::new(screen, &mut self.cache);

        match scene.state.overlay() {
            Overlay::About => draw_about(&mut screen, scene),
            Overlay::Menu => {
                self.layers.draw(&mut screen, scene);
                look::menu(
                    &mut screen,
                    scene.model.settings().about().is_some(),
                    scene.state.menu_seconds(),
                );
            }
            Overlay::None => self.layers.draw(&mut screen, scene),
        }
    }

    /// Like [`Self::render`], onto a new screen.
    #[cfg(test)]
    pub fn rendered(&mut self, scene: Scene, scale: f32) -> Pixmap {
        let mut screen = blank(screen_size(scale));

        self.render(screen.as_mut(), scene, scale);
        screen
    }
}

/// Like [`Renderer::render`], without caching.
#[must_use]
#[cfg(test)]
pub fn render(scene: Scene, scale: f32) -> Pixmap {
    Renderer::default().rendered(scene, scale)
}
