//! The preview's drawings. Fixed ones ([`Art`]) are SVG, rasterized once per scale ([`draw`]);
//! value-driven ones are paths drawn straight onto the canvas ([`knob_arc`], [`fader`]).

use std::{f32::consts::PI, fmt::Write as _};

use crabgram_styling::host;
use tiny_skia::{
    FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

use super::canvas::Canvas;

/// A fixed drawing: what the cache of [`draw`] is keyed by, so its SVG is made on a miss only.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Art {
    /// A widget card `width` wide.
    Card { width: u32 },
    /// The dark plate behind the top bar's bypass switch.
    BypassPlate,
    /// The shadow the top bar casts on the content below it.
    BarShadow,
    /// The track of a switch, lit when `on`.
    SwitchTrack { on: bool },
    /// The knob of a switch with its shadow.
    SwitchKnob,
    /// A knob's ring, unlit.
    KnobTrack,
    /// A pagination dot: the current page's is larger and light.
    Dot { current: bool },
    /// The box of the scene in the top bar.
    SceneBox,
    /// A line icon in a color.
    Icon { icon: Icon, color: (u8, u8, u8) },
}

fn rasterize(svg: &str, (width, height): (f32, f32), scale: f32) -> Pixmap {
    let document = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' \
         viewBox='0 0 {width} {height}'>{svg}</svg>"
    );
    let tree = resvg::usvg::Tree::from_str(&document, &resvg::usvg::Options::default())
        .expect("the drawings are valid SVG");
    let mut pixmap = Pixmap::new(
        ((width * scale).ceil() as u32).max(1),
        ((height * scale).ceil() as u32).max(1),
    )
    .expect("a drawing has a positive size");

    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
}

/// Draws `art` with its top left corner at `at`.
pub fn draw(canvas: &mut Canvas<'_>, art: Art, at: (f32, f32)) {
    let (x, y) = canvas.on_pixmap(canvas.device_pixel(at));
    let scale = canvas.scale();
    let (pixmap, cache) = canvas.parts();
    let drawing = cache.art.entry(art).or_insert_with(|| {
        let (svg, size) = art.svg();

        rasterize(&svg, size, scale)
    });

    pixmap.draw_pixmap(
        x,
        y,
        drawing.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

impl Art {
    /// The SVG and its size in screen pixels.
    fn svg(self) -> (String, (f32, f32)) {
        match self {
            Self::Card { width } => plate(width as f32, &CARD),
            Self::BypassPlate => plate(host::BYPASS.0 as f32, &BYPASS_PLATE),
            Self::BarShadow => bar_shadow(),
            Self::SwitchTrack { on } => switch_track(on),
            Self::SwitchKnob => switch_knob(),
            Self::KnobTrack => knob_track(),
            Self::Dot { current } => dot(current),
            Self::SceneBox => scene_box(),
            Self::Icon { icon, color } => (icon_art(icon, color), icon.size()),
        }
    }
}

/// A blur filter `id` of `deviation` pixels.
fn blur(id: &str, deviation: f32) -> String {
    format!(
        "<filter id='{id}' x='-50%' y='-50%' width='200%' height='200%'>\
         <feGaussianBlur stdDeviation='{deviation}'/></filter>"
    )
}

/// A point `radius` from `(cx, cy)` at `degrees`, clockwise from the right.
fn polar((cx, cy): (f32, f32), radius: f32, degrees: f32) -> (f32, f32) {
    let radians = degrees * PI / 180.0;

    (cx + radius * radians.cos(), cy + radius * radians.sin())
}

/// The path of a clockwise arc around `center` from `from` to `to` degrees, 0 pointing right.
pub fn arc(center: (f32, f32), radius: f32, (from, to): (f32, f32)) -> String {
    let ((x0, y0), (x1, y1)) = (polar(center, radius, from), polar(center, radius, to));
    let large = u8::from((to - from).rem_euclid(360.0) > 180.0);

    format!("M{x0:.3} {y0:.3}A{radius} {radius} 0 {large} 1 {x1:.3} {y1:.3}")
}

fn svg_gray(level: u8) -> String {
    format!("rgb({level},{level},{level})")
}

/// One light or shadow edge of a [`Plate`]: a blurred stroke around the plate, moved by `offset`.
struct Edge {
    offset: (f32, f32),
    opacity: f32,
    stroke: f32,
    blur: f32,
}

/// A dark plate, shaded inside with a shadow edge and a light edge, `margin` wider than drawn.
struct Plate {
    height: f32,
    radius: f32,
    margin: f32,
    shadow: Edge,
    light: Edge,
}

const CARD: Plate = Plate {
    height: host::WIDGET.1 as f32,
    radius: 19.94,
    margin: 2.0,
    shadow: Edge {
        offset: (-0.56, 1.94),
        opacity: 0.3125,
        stroke: 5.75,
        blur: 1.406,
    },
    light: Edge {
        offset: (-1.44, -1.91),
        opacity: 0.03,
        stroke: 5.0,
        blur: 1.469,
    },
};

const BYPASS_PLATE: Plate = Plate {
    height: host::BYPASS.1 as f32,
    radius: 9.875,
    margin: 0.0,
    shadow: Edge {
        offset: (-0.59, 1.88),
        opacity: 0.4125,
        stroke: 5.0,
        blur: 1.344,
    },
    light: Edge {
        offset: (-2.22, -2.09),
        opacity: 0.01,
        stroke: 4.0,
        blur: 0.969,
    },
};

/// A plate `width` wide.
fn plate(width: f32, plate: &Plate) -> (String, (f32, f32)) {
    let Plate {
        height,
        radius,
        margin,
        ref shadow,
        ref light,
    } = *plate;
    let edge = |id: &str, color: &str, edge: &Edge| {
        format!(
            "<rect x='{}' y='{}' width='{width}' height='{height}' rx='{radius}' fill='none' \
             stroke='{color}' stroke-opacity='{}' stroke-width='{}' filter='url(#{id})'/>",
            edge.offset.0, edge.offset.1, edge.opacity, edge.stroke,
        )
    };

    (
        format!(
            "<defs><clipPath id='c'><rect width='{width}' height='{height}' rx='{radius}'/>\
             </clipPath>{}{}</defs><g clip-path='url(#c)'><rect width='{width}' height='{height}' \
             fill='#1a1a1a'/>{}{}</g>",
            blur("s", shadow.blur),
            blur("h", light.blur),
            edge("s", "#000", shadow),
            edge("h", "#fff", light),
        ),
        (width + margin, height),
    )
}

fn bar_shadow() -> (String, (f32, f32)) {
    let (width, height) = (host::TOP_BAR.0 as f32, 17.0);

    (
        format!(
            "<defs><linearGradient id='g' x1='0' y1='0' x2='0' y2='1'>\
             <stop offset='0' stop-color='#0d0d0d' stop-opacity='0.6775'/>\
             <stop offset='0.4437' stop-color='#0d0d0d' stop-opacity='0.0376'/>\
             <stop offset='1' stop-color='#0d0d0d' stop-opacity='0'/></linearGradient></defs>\
             <rect width='{width}' height='16.375' fill='url(#g)'/>"
        ),
        (width, height),
    )
}

fn switch_track(on: bool) -> (String, (f32, f32)) {
    let level = if on { 194 } else { 51 };

    (
        format!(
            "<defs><clipPath id='c'><rect width='87' height='35' rx='17.16'/></clipPath>{}</defs>\
             <rect width='87' height='35' rx='17.5' fill='{}'/><g clip-path='url(#c)'>\
             <rect x='-0.94' y='0.94' width='87' height='35' rx='17.16' fill='none' stroke='#000' \
             stroke-opacity='0.375' stroke-width='2.44' filter='url(#b)'/></g>",
            blur("b", 0.641),
            svg_gray(level),
        ),
        (87.0, 35.0),
    )
}

fn switch_knob() -> (String, (f32, f32)) {
    (
        format!(
            "<defs>{}</defs><circle cx='22.06' cy='22.05' r='20.52' fill='#000' \
             fill-opacity='0.3125' filter='url(#b)'/><circle cx='21.52' cy='21.52' r='20.52' \
             fill='#858585'/>",
            blur("b", 0.6875),
        ),
        (44.0, 44.0),
    )
}

const KNOB_CENTER: (f32, f32) = (44.0, 44.0);
const KNOB_SIZE: (f32, f32) = (88.0, 88.0);

fn knob_track() -> (String, (f32, f32)) {
    (
        format!(
            "<defs>{}{}</defs><path d='{}' stroke='#333333' stroke-width='8.79' \
             stroke-linecap='round' fill='none'/><path d='{}' stroke='#000' stroke-opacity='0.3' \
             stroke-width='1.56' stroke-linecap='round' fill='none' filter='url(#o)'/><path \
             d='{}' stroke='#000' stroke-opacity='0.1625' stroke-width='1.125' \
             stroke-linecap='round' fill='none' filter='url(#i)'/>",
            blur("o", 0.4),
            blur("i", 0.325),
            arc(KNOB_CENTER, 39.64, (132.5, 407.875)),
            arc(KNOB_CENTER, 43.25, (132.25, 406.9)),
            arc(KNOB_CENTER, 35.84, (132.25, 406.9)),
        ),
        KNOB_SIZE,
    )
}

/// The transform that draws screen pixels at `at` onto `canvas`.
fn placed(canvas: &Canvas<'_>, (x, y): (f32, f32)) -> Transform {
    canvas.transform().pre_translate(x, y)
}

fn solid(red: u8, green: u8, blue: u8) -> Paint<'static> {
    let mut paint = Paint::default();

    paint.set_color_rgba8(red, green, blue, 255);
    paint.anti_alias = true;
    paint
}

/// A clockwise arc around `center` from `from` to `to` degrees, 0 pointing right, in cubic pieces
/// of at most a quarter turn.
fn arc_path(center: (f32, f32), radius: f32, (from, to): (f32, f32)) -> Option<Path> {
    let pieces = ((to - from) / 90.0).ceil().max(1.0);
    let step = (to - from) / pieces;
    let handle = radius * 4.0 / 3.0 * (step.to_radians() / 4.0).tan();
    let tangent = |degrees: f32| {
        let (sin, cos) = degrees.to_radians().sin_cos();

        (-sin * handle, cos * handle)
    };
    let mut path = PathBuilder::new();
    let (x0, y0) = polar(center, radius, from);

    path.move_to(x0, y0);
    (0..pieces as u32)
        .map(|piece| from + step * piece as f32)
        .for_each(|start| {
            let end = start + step;
            let ((x0, y0), (x1, y1)) = (polar(center, radius, start), polar(center, radius, end));
            let ((dx0, dy0), (dx1, dy1)) = (tangent(start), tangent(end));

            path.cubic_to(x0 + dx0, y0 + dy0, x1 - dx1, y1 - dy1, x1, y1);
        });
    path.finish()
}

/// A rectangle with corners rounded as SVG's `rx` rounds them: `radius` wide, at most half the
/// width and half the height.
pub(super) fn rounded(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Option<Path> {
    const KAPPA: f32 = 0.552_284_8;

    let (rx, ry) = (radius.min(width / 2.0), radius.min(height / 2.0));
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    let (right, bottom) = (x + width, y + height);
    let mut path = PathBuilder::new();

    path.move_to(x + rx, y);
    path.line_to(right - rx, y);
    path.cubic_to(right - rx + kx, y, right, y + ry - ky, right, y + ry);
    path.line_to(right, bottom - ry);
    path.cubic_to(
        right,
        bottom - ry + ky,
        right - rx + kx,
        bottom,
        right - rx,
        bottom,
    );
    path.line_to(x + rx, bottom);
    path.cubic_to(x + rx - kx, bottom, x, bottom - ry + ky, x, bottom - ry);
    path.line_to(x, y + ry);
    path.cubic_to(x, y + ry - ky, x + rx - kx, y, x + rx, y);
    path.close();
    path.finish()
}

/// Draws the lit part of a knob's ring over [`knob_track`] at `at`, from its start to `value`
/// (0 to 1); nothing at 0.
pub fn knob_arc(canvas: &mut Canvas<'_>, value: f32, at: (f32, f32)) {
    let Some(path) = (value > 0.0)
        .then(|| {
            arc_path(
                KNOB_CENTER,
                40.13,
                (135.0, 270.0f32.mul_add(value.min(1.0), 135.0)),
            )
        })
        .flatten()
    else {
        return;
    };
    let stroke = Stroke {
        width: 7.55,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };

    let transform = placed(canvas, at);

    canvas
        .pixmap()
        .stroke_path(&path, &solid(191, 191, 191), &stroke, transform, None);
}

/// Draws a vertical fader at `at`, its handle at `value`, 0 to 1.
pub fn fader(canvas: &mut Canvas<'_>, value: f32, at: (f32, f32)) {
    let (x, width, top, bottom): (f32, f32, f32, f32) = (13.34, 15.25, 16.44, 124.94);
    let handle = (top - bottom).mul_add(value.clamp(0.0, 1.0), bottom);
    let radius = width / 2.0;
    let (handle_width, handle_height) = (27.94, 14.19);
    let middle = x + radius;
    let transform = placed(canvas, at);
    let shapes = [
        (
            rounded(x, top - radius, width, bottom - top + width, radius),
            solid(51, 51, 51),
        ),
        (
            rounded(
                x,
                handle,
                width,
                (bottom + radius - handle).max(0.01),
                radius,
            ),
            solid(194, 194, 194),
        ),
        (
            rounded(
                middle - handle_width / 2.0,
                handle - handle_height / 2.0,
                handle_width,
                handle_height,
                handle_height / 2.0,
            ),
            solid(222, 222, 222),
        ),
        (
            rounded(middle - 9.0, handle - 0.5, 18.0, 1.0, 0.0),
            solid(35, 35, 35),
        ),
    ];

    for (path, paint) in shapes {
        if let Some(path) = path {
            canvas
                .pixmap()
                .fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
    }
}

fn dot(current: bool) -> (String, (f32, f32)) {
    let (stops, size, radius) = if current {
        (
            "<stop offset='0' stop-color='rgb(85,85,85)'/>\
             <stop offset='0.2437' stop-color='rgb(194,194,194)'/>\
             <stop offset='1' stop-color='rgb(195,195,195)'/>",
            11.0,
            5.47,
        )
    } else {
        (
            "<stop offset='0' stop-color='rgb(0,0,0)'/>\
             <stop offset='1' stop-color='rgb(55,55,55)'/>",
            9.0,
            4.52,
        )
    };
    let middle = size / 2.0;

    (
        format!(
            "<defs><linearGradient id='g' x1='0' y1='0' x2='0' y2='1'>{stops}</linearGradient>\
             </defs><circle cx='{middle}' cy='{middle}' r='{radius}' fill='url(#g)'/>"
        ),
        (size, size),
    )
}

/// A shape whose top left and bottom right corners round by `large`, the others by `small`.
fn tab((x, y): (f32, f32), (width, height): (f32, f32), (large, small): (f32, f32)) -> String {
    format!(
        "M{} {y}H{}A{small} {small} 0 0 1 {} {}V{}A{large} {large} 0 0 1 {} {}H{}\
         A{small} {small} 0 0 1 {x} {}V{}A{large} {large} 0 0 1 {} {y}Z",
        x + large,
        x + width - small,
        x + width,
        y + small,
        y + height - large,
        x + width - large,
        y + height,
        x + small,
        y + height - small,
        y + large,
        x + large,
    )
}

fn scene_box() -> (String, (f32, f32)) {
    let path = tab((1.0, 1.0), (98.0, 48.0), (18.94, 4.5));

    (
        format!(
            "<defs><clipPath id='c'><path d='{path}'/></clipPath>{}</defs>\
             <path d='{path}' fill='#09334b'/><g clip-path='url(#c)'><path d='{path}' \
             transform='translate(0 2.375)' fill='none' stroke='#000' stroke-opacity='0.25' \
             stroke-width='6' filter='url(#b)'/></g><path d='{path}' fill='none' \
             stroke='#1c516f' stroke-width='2'/>",
            blur("b", 0.75),
        ),
        (100.0, 50.0),
    )
}

/// The line icons of the top bar and of the "⋮" menu.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Icon {
    Close,
    Back,
    Remove,
    Swap,
    More,
    Settings,
    Save,
    Info,
    Reset,
    Restore,
}

/// A clockwise arc from `from` to `to` degrees ending in an arrowhead `head` long, its sides
/// `spread` degrees off the arc's direction.
fn arrow_arc(
    center: (f32, f32),
    radius: f32,
    (from, to): (f32, f32),
    (head, spread): (f32, f32),
    stroke: &str,
) -> String {
    let (x, y) = polar(center, radius, to);
    let side = |offset: f32| {
        let (dx, dy) = polar((0.0, 0.0), head, to + 90.0 + offset);

        (x - dx, y - dy)
    };
    let ((x1, y1), (x2, y2)) = (side(-spread), side(spread));

    format!(
        "<path d='{}' {stroke} stroke-linecap='round'/><path d='M{x1:.3} {y1:.3}L{x:.3} {y:.3}\
         L{x2:.3} {y2:.3}' {stroke} stroke-linecap='round' stroke-linejoin='round'/>",
        arc(center, radius, (from, to)),
    )
}

/// A gear of six rounded lobes around `center`.
fn gear(center: (f32, f32), stroke: &str) -> String {
    let lobe = |degrees: f32| {
        let wave = 2.97 * (6.0 * degrees * PI / 180.0).cos();

        12.375 - wave.tanh() / 2.97f32.tanh()
    };
    let mut outline = String::new();

    for degrees in 0..360u16 {
        let degrees = f32::from(degrees);
        let (x, y) = polar(center, lobe(degrees), degrees);
        let _ = write!(
            outline,
            "{}{x:.3} {y:.3}",
            if degrees == 0.0 { 'M' } else { 'L' }
        );
    }

    format!(
        "<path d='{outline}Z' {stroke} stroke-linejoin='round'/>\
         <circle cx='{}' cy='{}' r='3.8' {stroke}/>",
        center.0, center.1,
    )
}

impl Icon {
    /// The size of the drawing.
    pub const fn size(self) -> (f32, f32) {
        match self {
            Self::Reset => (40.0, 40.0),
            Self::Restore => (26.0, 22.0),
            _ => (30.0, 30.0),
        }
    }

    /// Where the drawing sits from the top left corner of the "⋮" menu item holding it.
    pub const fn menu_offset(self) -> (f32, f32) {
        match self {
            Self::Reset => (10.0, 10.0),
            Self::Restore => (15.0, 19.0),
            _ => (15.0, 15.0),
        }
    }
}

/// The SVG of `icon` in `color`.
fn icon_art(icon: Icon, (r, g, b): (u8, u8, u8)) -> String {
    let color = format!("rgb({r},{g},{b})");
    let bar = format!("stroke='{color}' fill='none' stroke-linecap='butt'");
    let menu = |width: f32| format!("stroke='{color}' stroke-width='{width}' fill='none'");

    match icon {
        Icon::Close => {
            format!("<path d='M0.48 0.48L29.5 29.5M29.5 0.48L0.48 29.5' {bar} stroke-width='2.5'/>")
        }
        Icon::Back => format!(
            "<path d='M24.69 0.53L6.31 15L24.69 29.5' {bar} stroke-width='2.52' \
                 stroke-linejoin='miter'/>"
        ),
        Icon::Remove => format!(
            "<g {bar} stroke-width='1.75' stroke-linejoin='miter'><path d='M1.75 6H28'/>\
                 <path d='M10.25 6V1.25H19.25V6'/>\
                 <path d='M5.53 9.25L7.5 28.42H22.28L24.2 9.25'/></g>"
        ),
        Icon::Swap => format!(
            "<g {bar} stroke-width='2.44' stroke-linejoin='miter'><path d='M0.25 7.5H29.75'/>\
                 <path d='M21.75 0.5L28.78 7.5L21.75 14.5'/><path d='M29.75 22.5H0.25'/>\
                 <path d='M8.28 15.5L1.06 22.5L8.28 29.53'/></g>"
        ),
        Icon::More => format!(
            "<g fill='{color}'><circle cx='15' cy='6' r='3.03'/>\
                 <circle cx='15' cy='15' r='3.03'/><circle cx='15' cy='24' r='3.03'/></g>"
        ),
        Icon::Settings => gear((14.875, 14.875), &menu(1.75)),
        Icon::Save => format!(
            "<g {} stroke-linejoin='miter'><path d='M4 3.75H21L26 8.75V26H4Z'/>\
                 <path d='M9.75 3.75V10.25H20.75V3.75'/></g>",
            menu(2.0),
        ),
        Icon::Info => format!(
            "<circle cx='15' cy='15' r='13.77' {0}/><path d='M15 10.25V24.75' {0}/>\
                 <circle cx='15' cy='6.25' r='1.25' fill='{color}'/>",
            menu(2.5),
        ),
        Icon::Reset => arrow_arc(
            (19.94, 21.81),
            9.625,
            (8.125, 287.5),
            (2.66, 47.5),
            &menu(1.75),
        ),
        Icon::Restore => format!(
            "{}<circle cx='10.72' cy='10.5' r='2.33' {}/>",
            arrow_arc(
                (10.75, 10.66),
                9.875,
                (105.0, 375.0),
                (2.41, 42.8),
                &menu(1.75),
            ),
            menu(1.75),
        ),
    }
}

#[cfg(test)]
mod tests {
    use tiny_skia::Pixmap;

    use super::super::{cache::Cache, canvas::Canvas, look};

    #[test]
    fn a_turning_knob_or_a_moving_fader_rasterizes_its_fixed_parts_once() {
        let mut pixmap = Pixmap::new(400, 200).unwrap();
        let mut cache = Cache::default();
        let mut canvas = Canvas::new(pixmap.as_mut(), &mut cache);
        let mut turn = |value: f32| {
            look::knob(&mut canvas, (0.0, 0.0), value, "Rate", ("1.0", ""), false);
            look::fader(
                &mut canvas,
                (200.0, 0.0),
                value,
                "Level",
                ("1.0", ""),
                false,
            );
            canvas.cache().art.len()
        };
        let after_one = turn(0.5);

        (0..=100).for_each(|step| {
            assert_eq!(turn(step as f32 / 100.0), after_one);
        });
    }
}
