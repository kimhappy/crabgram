//! The built-in widgets of the preview: a control without styling images of its own.

use core::{fmt::Write as _, mem};

use crabgram::{Control, ControlState, Kind, Unit, WidgetKind};
use crabgram_styling::{Framing, HostStyle, Parameter, Slot, host};
use tiny_skia::{Color, FillRule, IntRect, Mask, Pixmap, PixmapPaint, Transform};

use super::{
    cache::Cache,
    canvas::Canvas,
    chrome, look,
    renderer::{Images, Scene, paste, resized, scaled_frame},
};

/// How opaque a stock widget is once the plugin made it inactive or blocked.
const DIMMED: f32 = 25.0 / 255.0;

/// The label over a dimmed stock widget.
const fn state_label(state: ControlState) -> Option<&'static str> {
    match state {
        ControlState::Inactive => Some("Inactive"),
        ControlState::Blocked => Some("Blocked"),
        ControlState::Normal => None,
    }
}

/// Room around a widget that its drawing may reach, in screen pixels.
const SPILL: f32 = 8.0;

/// The device position and width of a stock widget's background.
pub type BackgroundSpot = ((i32, i32), u32);

/// The device pixels of the picture on `canvas` that a widget `width` wide at `(x, y)` may draw
/// on.
fn reach(canvas: &Canvas<'_>, (x, y): (f32, f32), width: u32) -> Option<IntRect> {
    let (origin_x, origin_y) = canvas.origin();
    let (canvas_width, canvas_height) = canvas.size();
    let edge = |screen: f32| (screen * canvas.scale()).round() as i32;
    let (left, top) = (edge(x - SPILL).max(origin_x), edge(y - SPILL).max(origin_y));
    let right = edge(x + width as f32 + SPILL).min(origin_x + canvas_width.cast_signed());
    let bottom =
        edge(y + host::WIDGET.1 as f32 + SPILL).min(origin_y + canvas_height.cast_signed());

    IntRect::from_ltrb(left, top, right, bottom)
}

/// The stock widget of `slot` at `place`, dimmed under a label while the plugin made it inactive
/// or blocked.
pub fn draw_widget(canvas: &mut Canvas<'_>, scene: Scene, slot: &Slot, place: (f32, f32)) {
    let state = scene.state().states()[slot.position()];
    let width = host::span_width(slot.slots());

    let Some(label) = state_label(state) else {
        widget(canvas, scene, slot, place, width);
        return;
    };

    if let Some(area) = reach(canvas, place, width) {
        let mut dimmed = canvas.cache().dimmed.take();

        if let Some(layer) = resized(&mut dimmed, (area.width(), area.height())) {
            let (x, y) = canvas.on_pixmap((area.x(), area.y()));

            layer.fill(Color::TRANSPARENT);
            widget(
                &mut Canvas::part(layer.as_mut(), (area.x(), area.y()), canvas.cache()),
                scene,
                slot,
                place,
                width,
            );
            canvas.pixmap().draw_pixmap(
                x,
                y,
                layer.as_ref(),
                &PixmapPaint {
                    opacity: DIMMED,
                    ..PixmapPaint::default()
                },
                Transform::identity(),
                None,
            );
        }

        canvas.cache().dimmed = dimmed;
    }

    look::state_label(canvas, label, place, width);
}

/// The stock widget of `slot`, `width` wide at `place`.
fn widget(canvas: &mut Canvas<'_>, scene: Scene, slot: &Slot, place: (f32, f32), width: u32) {
    let model = scene.model();
    let control = slot.control();
    let value = scene.state().values()[slot.position()];
    let style = model
        .settings()
        .parameter_of(control)
        .and_then(Parameter::style);
    let normalized = control.normalized(value);
    let caption = control.name();
    let mut number = mem::take(&mut canvas.cache().number);
    let unit = value_parts(&control, value, &mut number);
    let text = (number.as_str(), unit);
    let background = model.settings().stock_background(control);
    let with_card = background.is_none();

    if let Some(background) = background {
        stock_background(canvas, scene.images(), background.path(), place, width);
    }

    match (style, control.widget()) {
        (Some(HostStyle::Fader), _) | (None, WidgetKind::Meter) => {
            look::fader(canvas, place, normalized, caption, text, with_card);
        }
        (None, WidgetKind::Knob) => look::knob(canvas, place, normalized, caption, text, with_card),
        (None, WidgetKind::Toggle) => look::toggle(canvas, place, value > 0.0, caption, with_card),
        _ => look::list(
            canvas,
            place,
            width,
            control.labels(),
            control.label_index(value),
            caption,
            with_card,
        ),
    }

    canvas.cache().number = number;
}

/// Corner radius of the card a stock widget's background is clipped to.
const CARD_RADIUS: f32 = 20.0;

/// The default widget's background `path` behind a stock widget `width` wide at `(x, y)`: centred
/// on it and clipped to its rounded card.
fn stock_background(
    canvas: &mut Canvas<'_>,
    images: &Images,
    path: &str,
    (x, y): (f32, f32),
    width: u32,
) {
    let Some(image) = images.get(path) else {
        return;
    };
    let left = x as i32 + (width.cast_signed() - image.width().cast_signed()) / 2;
    let at = canvas.device_pixel((left as f32, y));
    let key = (at, width);
    let on_pixmap = canvas.on_pixmap(at);
    let (pixmap, cache) = canvas.parts();

    if let Some(background) = cache.clipped.get(path).and_then(|it| it.get(&key)) {
        paste(pixmap, background, on_pixmap);
    } else if let Some(background) = clip(cache, image, path, (x, y), width, at) {
        let background = cache
            .clipped
            .entry(path.to_owned())
            .or_default()
            .entry(key)
            .or_insert(background);

        paste(pixmap, background, on_pixmap);
    }
}

/// The frame of `image` at device pixel `at`, clipped to the rounded card of a stock widget `width`
/// wide at `(x, y)`; the frame is kept in `cache`.
fn clip(
    cache: &mut Cache,
    image: &Pixmap,
    path: &str,
    (x, y): (f32, f32),
    width: u32,
    at: (i32, i32),
) -> Option<Pixmap> {
    let scale = cache.scale();
    let mut clipped = scaled_frame(cache, image, path, Framing::Free, 0).clone();
    let mut mask = Mask::new(clipped.width(), clipped.height())?;
    let rounded = chrome::rounded(x, y, width as f32, host::WIDGET.1 as f32, CARD_RADIUS)?;

    mask.fill_path(
        &rounded,
        FillRule::Winding,
        true,
        Transform::from_translate(-at.0 as f32, -at.1 as f32).pre_scale(scale, scale),
    );
    clipped.apply_mask(&mask);
    Some(clipped)
}

/// Writes the number a stock widget shows into `number` and returns its unit.
fn value_parts(control: &Control, value: f32, number: &mut String) -> &'static str {
    number.clear();

    if matches!(control.kind(), Kind::Integer { .. }) {
        let _ = write!(number, "{value:.0}");

        return control.unit().map_or("", unit_suffix);
    }

    match control.unit() {
        Some(Unit::Hertz) if value >= 1_000.0 => {
            stock_number(value / 1_000.0, number);
            "kHz"
        }
        Some(unit @ (Unit::Decibel | Unit::OneDecimalPoint)) => {
            let _ = write!(number, "{value:.1}");

            unit_suffix(unit)
        }
        unit => {
            stock_number(value, number);
            unit.map_or("", unit_suffix)
        }
    }
}

/// Writes two decimals below 10, one below 100, none from 100 on, picked before rounding.
fn stock_number(value: f32, number: &mut String) {
    let decimals = match value.abs() {
        it if it < 10.0 => 2,
        it if it < 100.0 => 1,
        _ => 0,
    };
    let _ = write!(number, "{value:.decimals$}");
}

const fn unit_suffix(unit: Unit) -> &'static str {
    match unit {
        Unit::Cent => "ct",
        Unit::Decibel => "dB",
        Unit::Hertz => "Hz",
        Unit::KiloHertz => "kHz",
        Unit::MegaHertz => "MHz",
        Unit::Millisecond => "ms",
        Unit::OneDecimalPoint => "",
        Unit::Percent => "%",
        Unit::Second => "s",
        Unit::Semitone => "semi",
    }
}
