//! Built-in default widgets for the preview, in screen coordinates at the scale of the
//! [`Canvas`].
//!
//! Also the chrome around styled widgets: card, top bar buttons, edge meters, "⋮" menu.

use core::marker::PhantomData;

use crabgram_styling::{Button, host};
use tiny_skia::{Color, FillRule, LineCap, LineJoin, Paint, Path, Rect, Stroke};

use super::{
    canvas::Canvas,
    chrome::{self, Art, Icon},
    text::{self, Face, Placement},
};

/// A position on the screen, below the strip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint {
    pub x: f32,
    pub y: f32,
}

impl ScreenPoint {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A point of a coordinate space with [`Area`]s.
pub trait Point: Copy {
    fn xy(self) -> (f32, f32);
}

impl Point for ScreenPoint {
    fn xy(self) -> (f32, f32) {
        (self.x, self.y)
    }
}

/// A rectangle of the space of `P`, for what a touch can land on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area<P = ScreenPoint> {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    space: PhantomData<P>,
}

impl<P> Area<P> {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            space: PhantomData,
        }
    }

    /// The area of `size` at `place`.
    #[must_use]
    pub const fn at((x, y): (i32, i32), (width, height): (u32, u32)) -> Self {
        Self::new(x as f32, y as f32, width as f32, height as f32)
    }

    #[must_use]
    pub const fn x(self) -> f32 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> f32 {
        self.y
    }

    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> f32 {
        self.height
    }
}

impl<P: Point> Area<P> {
    #[must_use]
    pub fn contains(self, point: P) -> bool {
        let (x, y) = point.xy();

        (self.x..self.x + self.width).contains(&x) && (self.y..self.y + self.height).contains(&y)
    }
}

/// A host position as screen coordinates.
#[must_use]
pub const fn point((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32, y as f32)
}

fn gray(level: u8) -> Color {
    Color::from_rgba8(level, level, level, 0xff)
}

#[must_use]
pub fn accent() -> Color {
    gray(0xc2)
}

/// The color of panels, buttons and the screen's background.
#[must_use]
pub fn panel() -> Color {
    gray(0x1a)
}

#[must_use]
pub fn text_color() -> Color {
    gray(0xc0)
}

#[must_use]
pub fn muted() -> Color {
    gray(0x5e)
}

const BAR: u8 = 0x23;
const ICON: (u8, u8, u8) = (0xc2, 0xc2, 0xc2);
const SCENE_TEXT: (u8, u8, u8) = (0x35, 0xb3, 0xf9);
const SCENE_FILL: (u8, u8, u8) = (0x09, 0x34, 0x4c);
const TRASH: (u8, u8, u8) = (0xfa, 0x91, 0x87);
const SWITCH_FOCUS: (u8, u8, u8) = (0x35, 0xb4, 0xfb);
const WIDGET_FOCUS: (u8, u8, u8) = (0x46, 0xa6, 0xe8);

/// Fills the rectangle at `(x, y)` of `width` by `height`; nothing for an empty one.
pub fn fill_rect(canvas: &mut Canvas<'_>, x: f32, y: f32, width: f32, height: f32, color: Color) {
    let mut paint = Paint::default();

    paint.set_color(color);

    if let Some(area) = Rect::from_xywh(x, y, width, height) {
        let transform = canvas.transform();

        canvas.pixmap().fill_rect(area, &paint, transform, None);
    }
}

/// Fills every pixel of `canvas` with the background color.
pub fn clear(canvas: &mut Canvas<'_>) {
    canvas.pixmap().fill(panel());
}

fn fill_path(canvas: &mut Canvas<'_>, path: &Path, color: Color) {
    let mut paint = Paint::default();

    paint.set_color(color);
    paint.anti_alias = true;

    let transform = canvas.transform();

    canvas
        .pixmap()
        .fill_path(path, &paint, FillRule::Winding, transform, None);
}

pub fn fill_rounded(
    canvas: &mut Canvas<'_>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    color: Color,
) {
    if let Some(path) = chrome::rounded(x, y, width, height, radius) {
        fill_path(canvas, &path, color);
    }
}

fn stroke_path(canvas: &mut Canvas<'_>, path: &Path, color: Color, width: f32) {
    let mut paint = Paint::default();

    paint.set_color(color);
    paint.anti_alias = true;

    let transform = canvas.transform();

    canvas.pixmap().stroke_path(
        path,
        &paint,
        &Stroke {
            width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        },
        transform,
        None,
    );
}

fn card(canvas: &mut Canvas<'_>, (x, y): (f32, f32), width: u32) {
    chrome::draw(canvas, Art::Card { width }, (x - 1.0, y));
}

const FONT: f32 = 19.0;

fn centered(
    canvas: &mut Canvas<'_>,
    text: &str,
    size: f32,
    color: Color,
    (left, width): (f32, f32),
    baseline: f32,
) {
    text::draw(
        canvas,
        Face::Mono,
        text,
        size,
        color,
        Placement::mid(left, width, baseline),
    );
}

fn caption(canvas: &mut Canvas<'_>, text: &str, (left, width): (f32, f32), baseline: f32) {
    centered(canvas, text, FONT, text_color(), (left, width), baseline);
}

/// The room between a value and its unit: none without a unit.
fn unit_gap(unit: &str) -> f32 {
    if unit.is_empty() {
        0.0
    } else {
        text::measure(Face::Mono, FONT, " ") + 1.0
    }
}

/// `value` and a muted `unit` of font size `unit_size`, `gap` apart and centred together on
/// `middle`.
fn value_with_unit(
    canvas: &mut Canvas<'_>,
    (value, unit): (&str, &str),
    (unit_size, gap): (f32, f32),
    middle: f32,
    baseline: f32,
) {
    let (value_width, unit_width) = (
        text::measure(Face::Mono, FONT, value),
        text::measure(Face::Mono, unit_size, unit),
    );
    let left = middle - (value_width + gap + unit_width) / 2.0;

    text::draw(
        canvas,
        Face::Mono,
        value,
        FONT,
        text_color(),
        Placement::left(left, value_width, baseline),
    );
    text::draw(
        canvas,
        Face::Mono,
        unit,
        unit_size,
        muted(),
        Placement::left(left + value_width + gap, unit_width, baseline),
    );
}

/// A knob of the default widget size; `value` is 0 to 1.
pub fn knob(
    canvas: &mut Canvas<'_>,
    place: (f32, f32),
    value: f32,
    name: &str,
    text: (&str, &str),
    with_card: bool,
) {
    let (width, _) = host::WIDGET;
    let (left, top) = place;
    let center = (left + width as f32 / 2.0, top + 90.0);
    let ring = (left + 46.0, top + 51.0);

    if with_card {
        card(canvas, place, width);
    }
    caption(canvas, name, (left, width as f32), top + 33.0);
    chrome::draw(canvas, Art::KnobTrack, ring);

    chrome::knob_arc(canvas, value.clamp(0.0, 1.0), ring);

    match text {
        (number, "") => centered(
            canvas,
            number,
            FONT,
            text_color(),
            (left, width as f32),
            top + KNOB_SINGLE_BASELINE,
        ),
        (number, "%") => value_with_unit(
            canvas,
            (number, "%"),
            (SMALL_FONT, 0.0),
            center.0,
            top + KNOB_SINGLE_BASELINE,
        ),
        (number, unit) => {
            centered(
                canvas,
                number,
                FONT,
                text_color(),
                (left, width as f32),
                top + 89.0,
            );
            centered(
                canvas,
                unit,
                FONT,
                muted(),
                (left, width as f32),
                top + 116.0,
            );
        }
    }
}

/// Baseline of a knob's value without a unit below it.
const KNOB_SINGLE_BASELINE: f32 = 103.0;

/// Size of a percent sign beside a knob's value.
const SMALL_FONT: f32 = 15.0;

/// A switch whose track's top left corner is at `(left, top)`; the knob overhangs the track.
fn switch(canvas: &mut Canvas<'_>, (left, top): (f32, f32), on: bool) {
    let knob_x = if on { left + 45.0 } else { left - 1.0 };

    chrome::draw(canvas, Art::SwitchTrack { on }, (left, top));
    chrome::draw(canvas, Art::SwitchKnob, (knob_x, top - 5.0));
}

/// `label` over a dimmed stock widget at `(x, y)`, `width` wide.
pub fn state_label(canvas: &mut Canvas<'_>, label: &str, (x, y): (f32, f32), width: u32) {
    caption(canvas, label, (x, width as f32), y + 80.0);
}

fn switch_label(canvas: &mut Canvas<'_>, text: &str, lit: bool, at: Placement) {
    text::draw(
        canvas,
        Face::Mono,
        text,
        STATE_FONT,
        if lit { accent() } else { gray(0x6b) },
        at,
    );
}

pub fn toggle(canvas: &mut Canvas<'_>, place: (f32, f32), on: bool, name: &str, with_card: bool) {
    let (width, _) = host::WIDGET;
    let (left, top) = place;

    if with_card {
        card(canvas, place, width);
    }
    switch_label(
        canvas,
        "OFF",
        !on,
        Placement::right(left + 1.0, SWITCH_LABEL_WIDTH, top + 95.0),
    );
    switch_label(
        canvas,
        "ON",
        on,
        Placement::left(left + 146.0, SWITCH_LABEL_WIDTH, top + 95.0),
    );

    if !name.is_empty() {
        caption(canvas, name, (left, width as f32), top + 33.0);
    }

    switch(canvas, (left + 52.0, top + 72.0), on);
}

const STATE_FONT: f32 = 14.5;

const SWITCH_LABEL_WIDTH: f32 = 40.0;

/// A vertical fader; `value` is 0 to 1. Also an output control without an image of its own.
pub fn fader(
    canvas: &mut Canvas<'_>,
    place: (f32, f32),
    value: f32,
    name: &str,
    text: (&str, &str),
    with_card: bool,
) {
    let (width, _) = host::WIDGET;
    let (left, top) = place;
    if with_card {
        card(canvas, place, width);
    }

    chrome::fader(canvas, value, (left + 3.0, top + 2.0));

    let middle = left + 103.5;

    caption(canvas, name, (middle - 59.0, 120.0), top + 32.0);
    value_with_unit(canvas, text, (FONT, unit_gap(text.1)), middle, top + 97.0);
}

/// A list widget as wide as `width` (see [`host::span_width`]) showing a window of `labels`
/// around `selected`.
pub fn list(
    canvas: &mut Canvas<'_>,
    place: (f32, f32),
    width: u32,
    labels: &[&str],
    selected: usize,
    title: &str,
    with_card: bool,
) {
    let (left, top) = place;
    let pitch = ROW_HEIGHT;

    if with_card {
        card(canvas, place, width);
    }
    caption(canvas, title, (left, width as f32), top + 34.0);

    labels
        .iter()
        .enumerate()
        .map(|(index, label)| (index, label, index as f32 - selected as f32))
        .filter(|&(.., offset)| row_fits(offset))
        .for_each(|(index, label, offset)| {
            let row_top = top + ROW_TOP + pitch * offset;

            fill_rect(
                canvas,
                left + 15.0,
                row_top,
                width as f32 - 30.0,
                pitch,
                panel(),
            );

            if index == selected {
                fill_rounded(
                    canvas,
                    left + 15.0,
                    row_top,
                    width as f32 - 30.0,
                    pitch,
                    pitch / 2.0,
                    accent(),
                );
            }

            centered(
                canvas,
                label,
                FONT,
                if index == selected {
                    gray(0x37)
                } else {
                    muted()
                },
                (left, width as f32),
                row_top + 25.0,
            );
        });
}

/// Row height of a list widget.
pub const ROW_HEIGHT: f32 = 34.0;

/// Top edge of the selected row from the top of the widget.
pub const ROW_TOP: f32 = 73.0;

/// Highest top edge of a list row from the top of the widget, below the caption.
const ROWS_TOP: f32 = 37.0;

/// Lowest bottom edge of a list row from the top of the widget.
const ROWS_BOTTOM: f32 = host::WIDGET.1 as f32 - 4.0;

/// Whether the row `offset` rows below the selected one shows on a list widget.
#[must_use]
pub fn row_fits(offset: f32) -> bool {
    let row_top = ROW_TOP + ROW_HEIGHT * offset;

    row_top >= ROWS_TOP && row_top + ROW_HEIGHT <= ROWS_BOTTOM
}

/// `button` at `(x, y)` with `icon` centred in it.
fn button(canvas: &mut Canvas<'_>, (x, y): (f32, f32), button: Button, icon: Icon) {
    let (button_width, button_height) = button.frame();

    fill_rounded(
        canvas,
        x,
        y,
        button_width as f32,
        button_height as f32,
        5.0,
        panel(),
    );

    let color = if icon == Icon::Remove { TRASH } else { ICON };
    let (width, height) = icon.size();

    chrome::draw(
        canvas,
        Art::Icon { icon, color },
        (
            (x + (button_width as f32 - width) / 2.0).floor(),
            (y + (button_height as f32 - height) / 2.0).floor(),
        ),
    );
}

pub fn top_bar(canvas: &mut Canvas<'_>, name: &str, bypassed: bool, active_scene_only: bool) {
    let (width, height) = host::TOP_BAR;

    bar_background(canvas, width, height);

    button(
        canvas,
        point(host::position::CLOSE),
        Button::Close,
        Icon::Close,
    );

    let (name_x, name_y) = point(host::position::BLOCK_NAME);

    text::draw(
        canvas,
        Face::MonoMedium,
        name,
        35.0,
        text_color(),
        Placement::left(name_x, host::BLOCK_NAME.0 as f32, name_y + 41.0),
    );

    let (bypass_x, bypass_y) = point(host::position::BYPASS);

    chrome::draw(canvas, Art::BypassPlate, (bypass_x, bypass_y));
    switch(canvas, (bypass_x + 10.0, bypass_y + 8.0), !bypassed);

    scene_control(canvas, active_scene_only);

    let buttons = [
        (Button::Remove, Icon::Remove),
        (Button::Swap, Icon::Swap),
        (Button::More, Icon::More),
    ];

    for (&place, (kind, icon)) in host::position::BUTTONS.iter().zip(buttons) {
        button(canvas, point(place), kind, icon);
    }
}

/// What a focus ring is drawn around.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ring {
    Widget,
    /// The stock bypass switch.
    Switch,
    /// A styled bypass switch: thicker, rounder and paler.
    StyledSwitch,
}

impl Ring {
    const fn look(self) -> ((u8, u8, u8), f32, f32) {
        match self {
            Self::Widget => (WIDGET_FOCUS, 3.0, 18.5),
            Self::Switch => (SWITCH_FOCUS, 2.0, 9.0),
            Self::StyledSwitch => (WIDGET_FOCUS, 3.0, 14.0),
        }
    }
}

/// The focus ring drawn inside `area` once it is touched.
pub fn ring(canvas: &mut Canvas<'_>, area: Area, ring: Ring) {
    let ((red, green, blue), stroke, radius) = ring.look();
    let inset = stroke / 2.0;
    let path = chrome::rounded(
        area.x + inset,
        area.y + inset,
        area.width - stroke,
        area.height - stroke,
        radius,
    );

    if let Some(path) = path {
        stroke_path(
            canvas,
            &path,
            Color::from_rgba8(red, green, blue, 0xff),
            stroke,
        );
    }
}

fn bar_background(canvas: &mut Canvas<'_>, width: u32, height: u32) {
    fill_rect(canvas, 0.0, 0.0, width as f32, height as f32, gray(BAR));
    chrome::draw(canvas, Art::BarShadow, (0.0, height as f32));
}

/// Width of the scene number at the left of the scene button.
const SCENE_NUMBER: f32 = 120.0;

/// The fill of the scene button right of its number.
const SCENES: Area = {
    let (x, y) = point(host::position::SCENE_CONTROL);
    let (width, height) = host::SCENE_CONTROL;

    Area::new(
        x + SCENE_NUMBER,
        y,
        width as f32 - SCENE_NUMBER,
        height as f32,
    )
};

fn scene_control(canvas: &mut Canvas<'_>, active_scene_only: bool) {
    let color = |(r, g, b): (u8, u8, u8)| Color::from_rgba8(r, g, b, 0xff);

    let (x, y) = point(host::position::SCENE_CONTROL);

    chrome::draw(canvas, Art::SceneBox, (x, y));
    text::draw(
        canvas,
        Face::MonoMedium,
        "01A",
        35.0,
        color(SCENE_TEXT),
        Placement::mid(x, 100.0, 45.0),
    );

    fill_rounded(
        canvas,
        SCENES.x,
        SCENES.y,
        SCENES.width,
        SCENES.height,
        5.0,
        color(SCENE_FILL),
    );
    centered(
        canvas,
        if active_scene_only {
            "Control Active"
        } else {
            "Control All"
        },
        22.0,
        color(SCENE_TEXT),
        (SCENES.x, SCENES.width),
        38.0,
    );
}

/// The top bar of the about page: a back button, the name centred, nothing else.
pub fn back_bar(canvas: &mut Canvas<'_>, name: &str) {
    let (width, height) = host::TOP_BAR;

    bar_background(canvas, width, height);
    button(
        canvas,
        point(host::position::CLOSE),
        Button::Back,
        Icon::Back,
    );
    text::draw(
        canvas,
        Face::Mono,
        name,
        35.0,
        text_color(),
        Placement::mid(0.0, width as f32, 46.0),
    );
}

/// How far left of the screen's middle a styled row of pagination dots sits.
pub const DOTS_SHIFT: f32 = 2.0;

/// How far from a dot's middle a touch still picks it.
pub const DOT_REACH: f32 = 7.0;

/// Room between two stock dots.
const DOT_GAP: u32 = 5;

/// Vertical middle of the stock dots.
const DOT_MIDDLE: f32 = 269.5;

/// Width of a stock dot, larger for the `current` page.
const fn dot_size(current: bool) -> u32 {
    if current { 11 } else { 9 }
}

/// The left edge and width of each stock pagination dot, first page first.
pub fn dot_spans(pages: u32, current: u32) -> impl Iterator<Item = (f32, u32)> {
    let (screen_width, _) = host::SCREEN;
    let total = (0..pages)
        .map(|page| dot_size(page == current) + DOT_GAP)
        .sum::<u32>()
        .saturating_sub(DOT_GAP);
    let start = (screen_width.saturating_sub(total) / 2) as f32;

    (0..pages).scan(start, move |left, page| {
        let size = dot_size(page == current);
        let span = (*left, size);

        *left += (size + DOT_GAP) as f32;
        Some(span)
    })
}

/// The middle of each stock pagination dot, first page first.
pub fn dot_centers(pages: u32, current: u32) -> impl Iterator<Item = (f32, f32)> {
    dot_spans(pages, current).map(|(left, size)| (left + size as f32 / 2.0, DOT_MIDDLE))
}

pub fn dots(canvas: &mut Canvas<'_>, pages: u32, current: u32) {
    if pages <= 1 {
        return;
    }

    for (page, (left, size)) in dot_spans(pages, current).enumerate() {
        chrome::draw(
            canvas,
            Art::Dot {
                current: page as u32 == current,
            },
            (left, DOT_MIDDLE - size as f32 / 2.0 - 0.5),
        );
    }
}

/// The edge peak meters and "<70 dB" text, drawn on every page, styled or not.
pub fn edge_meters(canvas: &mut Canvas<'_>) {
    let (screen_width, _) = host::SCREEN;
    let caption_width = text::measure(Face::Mono, 15.0, "<70 dB");
    let baseline = 93.0;

    for (bar, text_left) in [
        (4.0, 25.0),
        (
            screen_width as f32 - 14.0,
            screen_width as f32 - 25.0 - caption_width,
        ),
    ] {
        fill_rounded(canvas, bar, 70.0, 10.0, 202.0, 4.0, gray(0x33));
        text::draw(
            canvas,
            Face::Mono,
            "<70 dB",
            15.0,
            gray(0x66),
            Placement::left(text_left, caption_width, baseline),
        );
    }
}

const MENU_PANEL: (f32, f32) = (764.0, 60.0);
const MENU_ITEM: (f32, f32) = (315.0, 60.0);
/// Space around and between menu items.
const MENU_GAP: f32 = 10.0;

/// What a "⋮" menu item does in a standalone UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Only closes the menu: "Configure quickpot" and "Save default state" have nothing to act
    /// on in a standalone UI.
    Inert,
    About,
    Reset,
}

/// An item of the "⋮" menu, at a `column` and `row` of its buttons; the last row has only the
/// right-hand column.
pub struct MenuItem {
    column: u32,
    row: u32,
    label: &'static str,
    icon: Icon,
    action: MenuAction,
}

impl MenuItem {
    #[must_use]
    pub const fn action(&self) -> MenuAction {
        self.action
    }

    /// The item's button.
    #[must_use]
    pub const fn area(&self) -> Area {
        let (panel_x, panel_y) = MENU_PANEL;
        let (width, height) = MENU_ITEM;

        Area::new(
            (width + MENU_GAP).mul_add(self.column as f32, panel_x + MENU_GAP),
            (height + MENU_GAP).mul_add(self.row as f32, panel_y + MENU_GAP),
            width,
            height,
        )
    }
}

static MENU_ITEMS: [MenuItem; 5] = [
    MenuItem {
        column: 0,
        row: 0,
        label: "Configure quickpot",
        icon: Icon::Settings,
        action: MenuAction::Inert,
    },
    MenuItem {
        column: 1,
        row: 0,
        label: "Save default state",
        icon: Icon::Save,
        action: MenuAction::Inert,
    },
    MenuItem {
        column: 0,
        row: 1,
        label: "About this block",
        icon: Icon::Info,
        action: MenuAction::About,
    },
    MenuItem {
        column: 1,
        row: 1,
        label: "Reset to default state",
        icon: Icon::Reset,
        action: MenuAction::Reset,
    },
    MenuItem {
        column: 1,
        row: 2,
        label: "Restore factory default state",
        icon: Icon::Restore,
        action: MenuAction::Reset,
    },
];

/// The "⋮" menu items shown: "About this block" only with an about page.
pub fn menu_items(has_about: bool) -> impl Iterator<Item = &'static MenuItem> {
    MENU_ITEMS
        .iter()
        .filter(move |item| has_about || item.action != MenuAction::About)
}

/// Flow speed in pixels per second, and the rest at either end in seconds.
const FLOW_SPEED: f32 = 51.0;
const FLOW_REST: f32 = 1.0;

/// A label `overflow` px too long rests, flows to its end, rests and jumps back.
fn flow_offset(overflow: f32, seconds: f32) -> f32 {
    let travel = overflow / FLOW_SPEED;
    let phase = seconds % (travel + 2.0 * FLOW_REST);

    (phase - FLOW_REST).clamp(0.0, travel) * FLOW_SPEED
}

/// `label` centred in `room` (left edge, width), or left-aligned and flowing when too long, cut
/// off at its edges.
fn label_flowing(
    canvas: &mut Canvas<'_>,
    label: &str,
    (left, room): (f32, f32),
    baseline: f32,
    seconds: f32,
) {
    const SIZE: f32 = 22.0;

    let overflow = text::measure(Face::Mono, SIZE, label) - room;
    let at = if overflow > 0.0 {
        Placement::left(left - flow_offset(overflow, seconds), room, baseline)
    } else {
        Placement::mid(left, room, baseline)
    };

    text::draw_within(
        canvas,
        Face::Mono,
        label,
        SIZE,
        text_color(),
        at,
        (left, left + room),
    );
}

/// The "⋮" menu over the dimmed screen; `seconds` drives the flow of long labels.
pub fn menu(canvas: &mut Canvas<'_>, has_about: bool, seconds: f32) {
    let (screen_width, screen_height) = host::SCREEN;
    let (panel_x, panel_y) = MENU_PANEL;

    fill_rect(
        canvas,
        0.0,
        0.0,
        screen_width as f32,
        screen_height as f32,
        Color::from_rgba8(0, 0, 0, 0xb6),
    );
    fill_rect(
        canvas,
        panel_x,
        panel_y,
        screen_width as f32 - panel_x,
        screen_height as f32 - panel_y,
        gray(0x33),
    );

    menu_items(has_about).for_each(|item| {
        let area = item.area();
        let (x, y, width, height) = (area.x, area.y, area.width, area.height);
        let (dx, dy) = item.icon.menu_offset();
        let icon = Art::Icon {
            icon: item.icon,
            color: ICON,
        };

        fill_rounded(canvas, x, y, width, height, 4.0, panel());
        chrome::draw(canvas, icon, (x + dx, y + dy));
        label_flowing(canvas, item.label, (x + 61.0, 239.0), y + 39.5, seconds);
    });
}
