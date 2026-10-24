//! The strip above the screen: input and output rows pick device, sample rate, buffer size and
//! channels; window scale and tempo sit beside them.

use std::borrow::Cow;

use crabgram::anagram::{MAX_BPM, MIN_BPM};
use crabgram_styling::host;

use crate::{
    audio::{self, Caps, DeviceChannel, Dir, Io, PerDir, Pick, PluginChannel, Settings},
    render::{
        canvas::Canvas,
        look::{self, Area, Point, ScreenPoint, fill_rounded},
        text::{self, Face, Placement},
    },
};

pub const HEIGHT: u16 = 88;

/// A position in the window's content: the strip, then the screen below it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentPoint {
    pub x: f32,
    pub y: f32,
}

impl ContentPoint {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// The same position on the screen below the strip.
    #[must_use]
    pub fn on_screen(self) -> ScreenPoint {
        ScreenPoint::new(self.x, self.y - f32::from(HEIGHT))
    }
}

impl Point for ContentPoint {
    fn xy(self) -> (f32, f32) {
        (self.x, self.y)
    }
}

const ROW_HEIGHT_OF_STRIP: f32 = 44.0;
const BOX_TOP: f32 = 6.0;
const BOX_HEIGHT: f32 = 32.0;
const DEVICE_LEFT: f32 = 16.0;
const DEVICE_WIDTH: f32 = 250.0;
const DEVICE_NAME_OFFSET: f32 = 96.0;
const RATE_LEFT: f32 = 278.0;
const RATE_WIDTH: f32 = 128.0;
const RATE_NAME_OFFSET: f32 = 64.0;
const BUFFER_LEFT: f32 = 418.0;
const BUFFER_WIDTH: f32 = 130.0;
const BUFFER_NAME_OFFSET: f32 = 82.0;
const CHANNELS_LEFT: f32 = 560.0;
const CHANNEL_PITCH: f32 = 102.0;
const CHANNEL_WIDTH: f32 = 90.0;
const CHANNEL_NAME_OFFSET: f32 = 54.0;
const MAX_CHANNEL_BOXES: usize = 4;
const SCALE_WIDTH: f32 = 110.0;
const SCALE_NAME_OFFSET: f32 = 76.0;
const BPM_WIDTH: f32 = 150.0;
const BPM_NAME_OFFSET: f32 = 58.0;
/// Room between the scale and the tempo box.
const BOX_GAP: f32 = 12.0;
const BOX_RADIUS: f32 = 8.0;
/// Distance from a box's left to its caption, and the room it has.
const CAPTION_OFFSET: f32 = 14.0;
const CAPTION_WIDTH: f32 = 80.0;
const CAPTION_SIZE: f32 = 18.0;
/// Distance from a box's right edge to the end of its value.
const VALUE_MARGIN: f32 = 10.0;
/// Baselines from the top of a box.
const CAPTION_BASELINE: f32 = 22.0;
const VALUE_BASELINE: f32 = 21.0;
const STATUS_SIZE: f32 = 14.0;
const VALUE_SIZE: f32 = 15.0;
/// Vertical drag pixels per BPM.
const PIXELS_PER_BPM: f32 = 3.0;
/// Width of `-` and `+` in the tempo box.
const STEP_WIDTH: f32 = 30.0;
const SCALES: [f32; 3] = [1.0, 1.5, 2.0];
const MIN_TEMPO: u16 = MIN_BPM as u16;
const MAX_TEMPO: u16 = MAX_BPM as u16;
const DROP_ROW_HEIGHT: f32 = 32.0;
const DROP_PADDING: f32 = 4.0;
const DROP_TOP: f32 = HEIGHT as f32 + DROP_PADDING;
const DROP_MIN_WIDTH: f32 = 220.0;
/// Distance from a list's sides to its labels.
const LABEL_INSET: f32 = 16.0;
const VISIBLE_ROWS: usize = 8;

/// A box of the strip.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Field {
    Device(Dir),
    Rate(Dir),
    Buffer(Dir),
    Channel(Dir, PluginChannel),
    Scale,
    Bpm,
}

/// Top edge of the boxes of strip row `row`.
const fn row_top(row: usize) -> f32 {
    BOX_TOP + ROW_HEIGHT_OF_STRIP * row as f32
}

/// Where a box sits in the strip, and where its value starts.
#[derive(Clone, Copy, Debug)]
struct Shape {
    left: f32,
    top: f32,
    width: f32,
    /// Distance from `left` to the value, right of the caption.
    name_offset: f32,
    /// Width of the value.
    room: f32,
}

impl Shape {
    fn contains(self, point: ContentPoint) -> bool {
        Area::new(self.left, self.top, self.width, BOX_HEIGHT).contains(point)
    }
}

impl Field {
    /// The box, given the number of channel boxes.
    fn shape(self, channel_boxes: usize) -> Shape {
        let right = CHANNELS_LEFT + CHANNEL_PITCH * channel_boxes as f32;
        let (row, left, width, name_offset) = match self {
            Self::Device(dir) => (dir.index(), DEVICE_LEFT, DEVICE_WIDTH, DEVICE_NAME_OFFSET),
            Self::Rate(dir) => (dir.index(), RATE_LEFT, RATE_WIDTH, RATE_NAME_OFFSET),
            Self::Buffer(dir) => (dir.index(), BUFFER_LEFT, BUFFER_WIDTH, BUFFER_NAME_OFFSET),
            Self::Channel(dir, PluginChannel(channel)) => (
                dir.index(),
                CHANNEL_PITCH.mul_add(channel as f32, CHANNELS_LEFT),
                CHANNEL_WIDTH,
                CHANNEL_NAME_OFFSET,
            ),
            Self::Scale => (0, right, SCALE_WIDTH, SCALE_NAME_OFFSET),
            Self::Bpm => (0, right + SCALE_WIDTH + BOX_GAP, BPM_WIDTH, BPM_NAME_OFFSET),
        };
        let right_margin = match self {
            Self::Bpm => 2.0 * STEP_WIDTH,
            _ => VALUE_MARGIN,
        };

        Shape {
            left,
            top: row_top(row),
            width,
            name_offset,
            room: width - name_offset - right_margin,
        }
    }

    fn caption(self) -> Cow<'static, str> {
        match self {
            Self::Device(Dir::Input) => "INPUT".into(),
            Self::Device(Dir::Output) => "OUTPUT".into(),
            Self::Rate(_) => "RATE".into(),
            Self::Buffer(_) => "BUFFER".into(),
            Self::Channel(_, PluginChannel(channel)) => format!("CH{}", channel + 1).into(),
            Self::Scale => "SCALE".into(),
            Self::Bpm => "BPM".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Change {
    /// `None` for silence.
    Input(Option<Pick>),
    Output(Pick),
    Rate(Dir, u32),
    Buffer(Dir, Option<u32>),
    Channel {
        dir: Dir,
        plugin: PluginChannel,
        device: DeviceChannel,
    },
    /// An index into the scales.
    Scale(usize),
    Bpm(u16),
}

impl Change {
    /// The scale this picks, if any.
    #[must_use]
    pub const fn scale(&self) -> Option<f32> {
        match *self {
            Self::Scale(index) => Some(SCALES[index]),
            _ => None,
        }
    }

    const fn field(&self) -> Field {
        match *self {
            Self::Input(_) => Field::Device(Dir::Input),
            Self::Output(_) => Field::Device(Dir::Output),
            Self::Rate(dir, _) => Field::Rate(dir),
            Self::Buffer(dir, _) => Field::Buffer(dir),
            Self::Channel { dir, plugin, .. } => Field::Channel(dir, plugin),
            Self::Scale(_) => Field::Scale,
            Self::Bpm(_) => Field::Bpm,
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Input(None) => "Off".to_owned(),
            Self::Input(Some(pick)) | Self::Output(pick) => pick.to_string(),
            Self::Rate(_, rate) => rate.to_string(),
            Self::Buffer(_, buffer) => audio::buffer_label(*buffer),
            Self::Channel {
                device: DeviceChannel(device),
                ..
            } => (device + 1).to_string(),
            Self::Scale(index) => format!("{}x", SCALES[*index]),
            Self::Bpm(bpm) => bpm.to_string(),
        }
    }

    #[must_use]
    pub const fn restarts_audio(&self) -> bool {
        !matches!(self, Self::Scale(_) | Self::Bpm(_))
    }
}

/// An open drop-down list under its box.
struct List {
    field: Field,
    entries: Vec<Change>,
    /// The entries as drawn.
    labels: Vec<String>,
    left: f32,
    /// Fits the widest label, within the screen.
    width: f32,
    /// The entry in the top row.
    scroll: usize,
}

impl List {
    /// The list of `entries` under the box of `field`, given the number of channel boxes.
    fn new(field: Field, entries: Vec<Change>, channel_boxes: usize) -> Self {
        let Shape { left, width, .. } = field.shape(channel_boxes);
        let labels: Vec<String> = entries.iter().map(Change::label).collect();
        let widest = labels
            .iter()
            .map(|label| text::measure(Face::Mono, VALUE_SIZE, label))
            .fold(0.0, f32::max);
        let width = (widest + 2.0 * LABEL_INSET)
            .max(width.max(DROP_MIN_WIDTH))
            .min(host::SCREEN.0 as f32 - left);
        let room = width - 2.0 * LABEL_INSET;

        Self {
            field,
            labels: labels
                .iter()
                .map(|label| text::fitted(Face::Mono, VALUE_SIZE, label, room).into_owned())
                .collect(),
            entries,
            left,
            width,
            scroll: 0,
        }
    }

    const fn last_scroll(&self) -> usize {
        self.entries.len().saturating_sub(VISIBLE_ROWS)
    }

    fn visible_rows(&self) -> usize {
        (self.entries.len() - self.scroll).min(VISIBLE_ROWS)
    }

    /// Where the list is drawn and pressed.
    fn area(&self) -> Area<ContentPoint> {
        Area::new(
            self.left,
            f32::from(HEIGHT),
            self.width,
            self.visible_rows() as f32 * DROP_ROW_HEIGHT + 2.0 * DROP_PADDING,
        )
    }

    /// The entry a press at `point` picks.
    fn entry_at(self, point: ContentPoint) -> Option<Change> {
        let row = ((point.y - DROP_TOP) / DROP_ROW_HEIGHT).floor();
        let inside =
            self.area().contains(point) && (0.0..self.visible_rows() as f32).contains(&row);

        inside
            .then(|| self.entries.into_iter().nth(self.scroll + row as usize))
            .flatten()
    }

    /// Positive steps scroll up.
    fn scroll(&mut self, steps: i8) {
        self.scroll = self
            .scroll
            .saturating_add_signed(-isize::from(steps))
            .min(self.last_scroll());
    }
}

/// A drag of the tempo: where it started and the tempo then.
#[derive(Clone, Copy, Debug)]
struct TempoDrag {
    start_y: f32,
    bpm: u16,
}

pub struct Strip {
    io: Io,
    /// The index of the scale the window shows.
    scale: usize,
    /// The tempo port as last read.
    bpm: u16,
    status: String,
    revision: u64,
    open: Option<List>,
    /// What the picked devices can do, as the audio thread last read them; none while it reads
    /// a newly picked one.
    caps: PerDir<Option<Caps>>,
    /// The device names as last listed.
    names: PerDir<Vec<String>>,
    plugin_channels: usize,
    drag: Option<TempoDrag>,
}

impl Strip {
    /// On the default devices, whose caps [`Self::show_fitted`] fills in.
    #[must_use]
    pub fn new(plugin_channels: usize) -> Self {
        Self {
            io: Io {
                input: Some(Settings::unread(Pick::Default, plugin_channels)),
                output: Settings::unread(Pick::Default, plugin_channels),
            },
            scale: 0,
            bpm: MIN_TEMPO,
            status: String::new(),
            revision: 0,
            open: None,
            caps: PerDir::default(),
            names: PerDir::default(),
            plugin_channels,
            drag: None,
        }
    }

    /// Shows what the devices of the last start sent can do, and the settings the audio thread
    /// fitted to them.
    pub fn show_fitted(&mut self, caps: PerDir<Caps>, io: Io) {
        let caps = caps.map(Some);

        if (&caps, &io) != (&self.caps, &self.io) {
            self.caps = caps;
            self.io = io;
            self.revise();
        }
    }

    #[must_use]
    pub const fn io(&self) -> &Io {
        &self.io
    }

    /// Shows the scale of SCALES nearest the window's `scale`.
    pub fn show_scale(&mut self, scale: f32) {
        let nearest = (0..SCALES.len())
            .min_by(|&a, &b| {
                (SCALES[a] - scale)
                    .abs()
                    .total_cmp(&(SCALES[b] - scale).abs())
            })
            .unwrap_or(0);

        if nearest != self.scale {
            self.scale = nearest;
            self.revise();
        }
    }

    /// Shows the tempo port's `bpm`.
    pub const fn show_tempo(&mut self, bpm: f32) {
        let bpm = bpm.round().clamp(MIN_BPM, MAX_BPM) as u16;

        if bpm != self.bpm {
            self.bpm = bpm;
            self.revise();
        }
    }

    /// Shows `status` beside the boxes.
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.revise();
    }

    /// Changes whenever what [`Self::draw_bar`] draws changes.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    const fn revise(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Changes the settings of `dir` by `change`, fitted to what its device can do as far as it
    /// was read.
    fn update(&mut self, dir: Dir, change: impl FnOnce(Settings) -> Settings) {
        if let Some(settings) = self.io.get_mut(dir) {
            let changed = change(settings.clone());

            *settings = match &self.caps[dir] {
                Some(caps) => changed.fitted(caps),
                None => changed,
            };
        }
    }

    fn channel_boxes(&self) -> usize {
        self.plugin_channels.min(MAX_CHANNEL_BOXES)
    }

    /// Applies `change`; whether it asks for anything new, which re-picking the current value
    /// never does. The scale and the tempo show once the window and the tempo port take them.
    pub fn apply(&mut self, change: &Change) -> bool {
        if self.is_current(change) {
            return false;
        }

        let unread = |pick: &Pick| Settings::unread(pick.clone(), self.plugin_channels);

        match change {
            Change::Input(pick) => {
                self.io.input = pick.as_ref().map(unread);
                self.caps[Dir::Input] = None;
            }
            Change::Output(pick) => {
                self.io.output = unread(pick);
                self.caps[Dir::Output] = None;
            }
            &Change::Rate(dir, rate) => self.update(dir, |it| it.with_rate(rate)),
            &Change::Buffer(dir, buffer) => self.update(dir, |it| it.with_buffer(buffer)),
            &Change::Channel {
                dir,
                plugin,
                device,
            } => self.update(dir, |it| it.with_channel(plugin, device)),
            Change::Scale(_) | Change::Bpm(_) => return true,
        }

        self.revise();
        true
    }

    fn fields(&self) -> impl Iterator<Item = Field> {
        let channel_boxes = self.channel_boxes();

        Dir::BOTH
            .into_iter()
            .flat_map(move |dir| {
                [Field::Device(dir), Field::Rate(dir), Field::Buffer(dir)]
                    .into_iter()
                    .chain(
                        (0..channel_boxes)
                            .map(move |channel| Field::Channel(dir, PluginChannel(channel))),
                    )
            })
            .chain([Field::Scale, Field::Bpm])
    }

    fn field_at(&self, point: ContentPoint) -> Option<Field> {
        let channel_boxes = self.channel_boxes();

        self.fields()
            .find(|field| field.shape(channel_boxes).contains(point))
    }

    /// Whether a press at `point` belongs to the strip: on the bar, or anywhere while a drop-down
    /// is open.
    #[must_use]
    pub fn covers(&self, point: ContentPoint) -> bool {
        point.y < f32::from(HEIGHT) || self.open.is_some()
    }

    /// What `field` shows; nothing for a setting of the silent input.
    fn current(&self, field: Field) -> Option<Change> {
        let settings = |dir| self.io.get(dir);

        match field {
            Field::Device(Dir::Input) => Some(Change::Input(
                self.io.input.as_ref().map(|it| it.pick().clone()),
            )),
            Field::Device(Dir::Output) => Some(Change::Output(self.io.output.pick().clone())),
            Field::Rate(dir) => settings(dir).map(|it| Change::Rate(dir, it.rate())),
            Field::Buffer(dir) => settings(dir).map(|it| Change::Buffer(dir, it.buffer())),
            Field::Channel(dir, plugin) => settings(dir).map(|it| Change::Channel {
                dir,
                plugin,
                device: it.channel(plugin),
            }),
            Field::Scale => Some(Change::Scale(self.scale)),
            Field::Bpm => Some(Change::Bpm(self.bpm)),
        }
    }

    fn is_current(&self, change: &Change) -> bool {
        self.current(change.field()).as_ref() == Some(change)
    }

    /// What `field` offers to pick; nothing for the tempo, which has no list, or for a setting of
    /// the silent input.
    fn entries(&self, field: Field) -> Vec<Change> {
        let names = |dir: Dir| self.names[dir].iter().cloned().map(Pick::Device);

        match field {
            Field::Device(Dir::Input) => [Some(Pick::Default), None]
                .into_iter()
                .chain(names(Dir::Input).map(Some))
                .map(Change::Input)
                .collect(),
            Field::Device(Dir::Output) => std::iter::once(Pick::Default)
                .chain(names(Dir::Output))
                .map(Change::Output)
                .collect(),
            Field::Rate(dir) | Field::Buffer(dir) | Field::Channel(dir, _) => {
                match (self.io.get(dir), &self.caps[dir]) {
                    (Some(settings), caps) => {
                        Self::settings_entries(field, settings, caps.as_ref())
                    }
                    (None, _) => Vec::new(),
                }
            }
            Field::Scale => (0..SCALES.len()).map(Change::Scale).collect(),
            Field::Bpm => Vec::new(),
        }
    }

    /// What a setting `field` of `settings` offers on a device that can do `caps`.
    fn settings_entries(field: Field, settings: &Settings, caps: Option<&Caps>) -> Vec<Change> {
        match field {
            Field::Rate(dir) => caps
                .into_iter()
                .flat_map(Caps::rates)
                .map(|&rate| Change::Rate(dir, rate))
                .collect(),
            Field::Buffer(dir) => std::iter::once(None)
                .chain(
                    caps.into_iter()
                        .flat_map(|caps| caps.buffers(settings))
                        .map(Some),
                )
                .map(|buffer| Change::Buffer(dir, buffer))
                .collect(),
            Field::Channel(dir, plugin) => {
                let devices = caps.map_or(0, |caps| caps.channels(settings.rate()));

                (0..devices)
                    .map(|device| Change::Channel {
                        dir,
                        plugin,
                        device: DeviceChannel(device),
                    })
                    .collect()
            }
            Field::Device(_) | Field::Scale | Field::Bpm => Vec::new(),
        }
    }

    /// The list of `field`; none without entries.
    fn list(&self, field: Field) -> Option<List> {
        let entries = self.entries(field);

        (!entries.is_empty()).then(|| List::new(field, entries, self.channel_boxes()))
    }

    /// A press while a list is open picks one of its entries, or closes it; on another box it
    /// goes on to that box.
    pub fn press(&mut self, point: ContentPoint) -> Option<Change> {
        let pressed = self.field_at(point);

        if let Some(list) = self.open.take() {
            let field = list.field;

            if let Some(entry) = list.entry_at(point) {
                return Some(entry);
            }
            if pressed.is_none_or(|it| it == field) {
                return None;
            }
        }

        match pressed? {
            Field::Bpm => Some(self.press_bpm(point)),
            field => {
                self.open = self.list(field);
                None
            }
        }
    }

    /// The tempo change by `steps` from `bpm`, within the tempo range.
    fn tempo(bpm: u16, steps: i32) -> Change {
        let bpm = (i32::from(bpm) + steps).clamp(i32::from(MIN_TEMPO), i32::from(MAX_TEMPO));

        Change::Bpm(u16::try_from(bpm).unwrap_or(MIN_TEMPO))
    }

    /// `-` and `+` step by one; anywhere else starts a drag.
    fn press_bpm(&mut self, point: ContentPoint) -> Change {
        let Shape { left, width, .. } = Field::Bpm.shape(self.channel_boxes());
        let right = left + width;
        let step = if point.x >= right - STEP_WIDTH {
            1
        } else if point.x >= right - 2.0 * STEP_WIDTH {
            -1
        } else {
            0
        };

        self.drag = (step == 0).then_some(TempoDrag {
            start_y: point.y,
            bpm: self.bpm,
        });
        Self::tempo(self.bpm, step)
    }

    #[must_use]
    pub const fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Up raises the tempo.
    pub fn drag(&self, point: ContentPoint) -> Option<Change> {
        self.drag.map(|TempoDrag { start_y, bpm }| {
            Self::tempo(bpm, ((start_y - point.y) / PIXELS_PER_BPM).round() as i32)
        })
    }

    pub const fn release(&mut self) {
        self.drag = None;
    }

    /// Closes the open list; whether one was open.
    pub fn close_list(&mut self) -> bool {
        self.open.take().is_some()
    }

    #[must_use]
    pub const fn is_listing(&self) -> bool {
        self.open.is_some()
    }

    /// Keeps the device `names`, showing them in the open device list; whether its entries
    /// changed.
    pub fn relist(&mut self, names: PerDir<Vec<String>>) -> bool {
        self.names = names;

        let Some(field) = self
            .open
            .as_ref()
            .map(|open| open.field)
            .filter(|field| matches!(field, Field::Device(_)))
        else {
            return false;
        };
        let entries = self.entries(field);
        let channel_boxes = self.channel_boxes();

        self.open.as_mut().is_some_and(|open| {
            let changed = open.entries != entries;
            let scroll = open.scroll;

            *open = List::new(field, entries, channel_boxes);
            open.scroll = scroll.min(open.last_scroll());
            changed
        })
    }

    /// Whether the open list is that of a device.
    #[must_use]
    pub const fn lists_devices(&self) -> bool {
        matches!(
            self.open,
            Some(List {
                field: Field::Device(_),
                ..
            })
        )
    }

    /// Scrolls the open list, else steps the box under `point`: the tempo by one, a setting by
    /// one entry. Positive steps scroll up, raise the tempo and step to the previous entry.
    pub fn wheel(&mut self, point: ContentPoint, steps: i8) -> Option<Change> {
        if let Some(list) = &mut self.open {
            list.scroll(steps);
            return None;
        }

        match self.field_at(point)? {
            Field::Device(_) => None,
            Field::Bpm => Some(Self::tempo(self.bpm, i32::from(steps)))
                .filter(|tempo| !self.is_current(tempo)),
            field => {
                let entries = self.entries(field);
                let index = entries.iter().position(|entry| self.is_current(entry))?;
                let next = index
                    .saturating_add_signed(-isize::from(steps))
                    .min(entries.len() - 1);

                (next != index).then(|| entries[next].clone())
            }
        }
    }

    fn selector(&self, canvas: &mut Canvas<'_>, field: Field) {
        let Shape {
            left,
            top,
            width,
            name_offset,
            room,
        } = field.shape(self.channel_boxes());
        let current = self.current(field);
        let (label, value_color) = current.as_ref().map_or_else(
            || ("-".to_owned(), look::muted()),
            |it| (it.label(), look::text_color()),
        );

        fill_rounded(
            canvas,
            left,
            top,
            width,
            BOX_HEIGHT,
            BOX_RADIUS,
            look::panel(),
        );
        text::draw(
            canvas,
            Face::Heading,
            &field.caption(),
            CAPTION_SIZE,
            look::muted(),
            Placement::left(left + CAPTION_OFFSET, CAPTION_WIDTH, top + CAPTION_BASELINE),
        );
        text::draw(
            canvas,
            Face::Mono,
            &text::fitted(Face::Mono, VALUE_SIZE, &label, room),
            VALUE_SIZE,
            value_color,
            Placement::left(left + name_offset, room, top + VALUE_BASELINE),
        );
    }

    fn steps(canvas: &mut Canvas<'_>, channel_boxes: usize) {
        let Shape {
            left, top, width, ..
        } = Field::Bpm.shape(channel_boxes);
        let right = left + width;

        for (label, from) in [("-", right - 2.0 * STEP_WIDTH), ("+", right - STEP_WIDTH)] {
            text::draw(
                canvas,
                Face::Mono,
                label,
                CAPTION_SIZE,
                look::text_color(),
                Placement::mid(from, STEP_WIDTH, top + CAPTION_BASELINE),
            );
        }
    }

    pub fn draw_bar(&self, canvas: &mut Canvas<'_>) {
        let width = host::SCREEN.0 as f32;
        let channel_boxes = self.channel_boxes();

        look::clear(canvas);
        for field in self.fields() {
            self.selector(canvas, field);
        }
        Self::steps(canvas, channel_boxes);

        let status_left = Field::Scale.shape(channel_boxes).left;

        text::draw(
            canvas,
            Face::Mono,
            &text::fitted(Face::Mono, STATUS_SIZE, &self.status, width - status_left),
            STATUS_SIZE,
            look::muted(),
            Placement::left(
                status_left,
                width - status_left,
                row_top(1) + VALUE_BASELINE,
            ),
        );
    }

    /// Draws the open list.
    pub fn draw_overlay(&self, canvas: &mut Canvas<'_>) {
        let Some(list) = &self.open else {
            return;
        };
        let area = list.area();
        let (left, top, width, height) = (area.x(), area.y(), area.width(), area.height());

        fill_rounded(canvas, left, top, width, height, BOX_RADIUS, look::panel());

        list.entries
            .iter()
            .zip(&list.labels)
            .skip(list.scroll)
            .take(VISIBLE_ROWS)
            .enumerate()
            .for_each(|(row, (entry, label))| {
                let y = DROP_TOP + row as f32 * DROP_ROW_HEIGHT;
                let chosen = self.is_current(entry);

                if chosen {
                    fill_rounded(
                        canvas,
                        left + 6.0,
                        y,
                        width - 12.0,
                        DROP_ROW_HEIGHT - 2.0,
                        6.0,
                        look::accent(),
                    );
                }

                text::draw(
                    canvas,
                    Face::Mono,
                    label,
                    VALUE_SIZE,
                    if chosen {
                        look::panel()
                    } else {
                        look::text_color()
                    },
                    Placement::left(
                        left + LABEL_INSET,
                        width - 2.0 * LABEL_INSET,
                        y + VALUE_BASELINE,
                    ),
                );
            });
    }
}

#[cfg(test)]
mod tests {
    use crabgram_styling::host;

    use super::{
        BOX_TOP, BUFFER_LEFT, Change, ContentPoint, DEVICE_LEFT, DROP_PADDING, DROP_ROW_HEIGHT,
        Face, Field, HEIGHT, MAX_CHANNEL_BOXES, MAX_TEMPO, MIN_TEMPO, RATE_LEFT,
        ROW_HEIGHT_OF_STRIP, STEP_WIDTH, Shape, Strip, VALUE_SIZE, text,
    };
    use crate::audio::{
        self, COMMON_BUFFERS, COMMON_RATES, Caps, DeviceChannel, Dir, PerDir, Pick, PluginChannel,
        Settings,
    };

    fn fits(field: Field, value: &str) -> bool {
        text::measure(Face::Mono, VALUE_SIZE, value) <= field.shape(MAX_CHANNEL_BOXES).room
    }

    #[test]
    fn every_tempo_fits_its_box_left_of_its_steps() {
        let Shape {
            left,
            width,
            name_offset,
            room,
            ..
        } = Field::Bpm.shape(2);

        assert!(fits(Field::Bpm, &MAX_TEMPO.to_string()));
        assert!(left + name_offset + room <= left + width - 2.0 * STEP_WIDTH);
    }

    #[test]
    fn every_common_rate_and_buffer_fits_its_box_whole() {
        let rates = COMMON_RATES.map(|rate| rate.to_string());
        let buffers = COMMON_BUFFERS
            .map(Some)
            .into_iter()
            .chain([None])
            .map(audio::buffer_label);

        assert!(rates.iter().all(|rate| fits(Field::Rate(Dir::Input), rate)));
        assert!(
            buffers
                .into_iter()
                .all(|buffer| fits(Field::Buffer(Dir::Output), &buffer))
        );
    }

    fn strip() -> Strip {
        let range = |(min, max)| {
            cpal::SupportedStreamConfigRange::new(
                4,
                cpal::SampleRate(min),
                cpal::SampleRate(max),
                cpal::SupportedBufferSize::Range { min: 64, max: 128 },
                cpal::SampleFormat::F32,
            )
        };
        let caps = Caps::of_ranges(&[range((44_100, 48_000)), range((96_000, 96_000))]);

        fitted(&caps)
    }

    /// A strip whose devices both can do `caps`, as the audio thread first fitted them.
    fn fitted(caps: &Caps) -> Strip {
        let mut strip = Strip::new(2);
        let caps = PerDir::from_fn(|_| caps.clone());
        let io = strip.io.clone().fitted(&caps);

        strip.show_fitted(caps, io);
        strip
    }

    fn input(strip: &Strip) -> &Settings {
        strip.io.input.as_ref().unwrap()
    }

    fn middle(left: f32, row: f32) -> ContentPoint {
        ContentPoint::new(left + 10.0, BOX_TOP + ROW_HEIGHT_OF_STRIP * row + 10.0)
    }

    fn row_y(row: usize) -> f32 {
        f32::from(HEIGHT) + DROP_PADDING + DROP_ROW_HEIGHT * row as f32 + 10.0
    }

    fn channel(dir: Dir, plugin: usize, device: usize) -> Change {
        Change::Channel {
            dir,
            plugin: PluginChannel(plugin),
            device: DeviceChannel(device),
        }
    }

    fn named(names: &[&str]) -> PerDir<Vec<String>> {
        PerDir::from_fn(|_| names.iter().map(|&name| name.to_owned()).collect())
    }

    #[test]
    fn picking_the_value_already_set_restarts_nothing() {
        let mut strip = strip();
        let rate = strip.io.output.rate();

        assert!(!strip.apply(&Change::Rate(Dir::Output, rate)));
        assert!(strip.apply(&Change::Rate(Dir::Output, 96_000)));
        assert_eq!(strip.io.output.rate(), 96_000);
        assert!(!strip.apply(&Change::Rate(Dir::Output, 96_000)));
        assert!(strip.apply(&Change::Buffer(Dir::Input, Some(64))));
        assert!(!strip.apply(&Change::Buffer(Dir::Input, Some(64))));
        assert!(strip.apply(&channel(Dir::Input, 1, 3)));
        assert_eq!(
            input(&strip).channels(),
            [DeviceChannel(0), DeviceChannel(3)]
        );
        assert!(!strip.apply(&channel(Dir::Input, 1, 3)));
        assert!(!strip.apply(&Change::Input(Some(Pick::Default))));
    }

    #[test]
    fn the_bar_is_revised_by_a_change_but_not_by_its_lists() {
        let mut strip = strip();
        let mut revised = {
            let mut seen = strip.revision();

            move |strip: &Strip| core::mem::replace(&mut seen, strip.revision()) != seen
        };

        strip.set_status("starting the audio");
        assert!(revised(&strip));

        assert_eq!(strip.press(middle(RATE_LEFT, 0.0)), None);
        assert_eq!(strip.wheel(middle(RATE_LEFT, 0.0), -1), None);
        assert!(!strip.apply(&Change::Bpm(MIN_TEMPO)));
        assert!(!revised(&strip));

        assert!(strip.apply(&Change::Rate(Dir::Input, 96_000)));
        assert!(revised(&strip));
    }

    #[test]
    fn a_buffer_the_new_mode_cannot_take_is_forgotten() {
        let range = |channels, buffers: (u32, u32)| {
            cpal::SupportedStreamConfigRange::new(
                channels,
                cpal::SampleRate(48_000),
                cpal::SampleRate(48_000),
                cpal::SupportedBufferSize::Range {
                    min: buffers.0,
                    max: buffers.1,
                },
                cpal::SampleFormat::F32,
            )
        };
        let mut strip = fitted(&Caps::of_ranges(&[
            range(2, (64, 64)),
            range(4, (256, 256)),
        ]));

        assert!(strip.apply(&Change::Buffer(Dir::Input, Some(64))));
        assert!(strip.apply(&channel(Dir::Input, 1, 3)));
        assert_eq!(input(&strip).buffer(), None);
    }

    #[test]
    fn a_newly_picked_device_shows_no_caps_and_opens_no_empty_list() {
        let mut strip = strip();
        let other = Pick::Device("other".to_owned());

        assert!(strip.apply(&Change::Rate(Dir::Input, 96_000)));
        assert!(strip.apply(&Change::Input(Some(other.clone()))));
        assert_eq!(*input(&strip), Settings::unread(other, 2));
        assert_eq!(strip.caps[Dir::Input], None);
        assert_eq!(strip.entries(Field::Rate(Dir::Input)), []);
        assert_eq!(strip.press(middle(RATE_LEFT, 0.0)), None);
        assert!(!strip.is_listing());
    }

    #[test]
    fn the_settings_of_the_silent_input_offer_nothing() {
        let mut strip = strip();

        assert!(strip.apply(&Change::Input(None)));

        for field in [
            Field::Rate(Dir::Input),
            Field::Buffer(Dir::Input),
            Field::Channel(Dir::Input, PluginChannel(0)),
        ] {
            let point = middle(field.shape(2).left, 0.0);

            assert_eq!(strip.current(field), None);
            assert_eq!(strip.press(point), None);
            assert!(!strip.is_listing(), "{field:?}");
            assert_eq!(strip.wheel(point, 1), None, "{field:?}");
        }

        assert!(strip.current(Field::Buffer(Dir::Output)).is_some());
    }

    #[test]
    fn retrying_the_same_device_keeps_the_settings_chosen() {
        let mut strip = strip();

        assert!(strip.apply(&Change::Rate(Dir::Input, 96_000)));
        assert!(strip.apply(&channel(Dir::Input, 1, 3)));

        let chosen = strip.io.clone();

        assert!(!strip.apply(&Change::Input(Some(Pick::Default))));
        assert_eq!(strip.io, chosen);
    }

    #[test]
    fn the_scale_and_the_tempo_show_what_the_window_and_the_port_took() {
        let mut strip = strip();

        assert!(strip.apply(&Change::Scale(2)));
        assert_eq!(strip.current(Field::Scale), Some(Change::Scale(0)));
        strip.show_scale(1.4);
        assert_eq!(strip.current(Field::Scale), Some(Change::Scale(1)));

        assert!(strip.apply(&Change::Bpm(120)));
        assert_eq!(strip.current(Field::Bpm), Some(Change::Bpm(MIN_TEMPO)));
        strip.show_tempo(120.0);
        assert_eq!(strip.current(Field::Bpm), Some(Change::Bpm(120)));
        assert!(!strip.apply(&Change::Bpm(120)));

        assert!(!Change::Scale(2).restarts_audio() && !Change::Bpm(120).restarts_audio());
    }

    #[test]
    fn a_list_opens_on_its_box_and_a_row_picks_its_entry() {
        let mut strip = strip();
        let ContentPoint { x, y } = middle(RATE_LEFT, 0.0);

        assert_eq!(strip.press(ContentPoint::new(x, y)), None);
        assert!(strip.is_listing());
        assert!(strip.covers(ContentPoint::new(x, 500.0)));
        assert_eq!(
            strip.press(ContentPoint::new(x, row_y(1))),
            Some(Change::Rate(Dir::Input, 48_000))
        );
        assert!(!strip.is_listing());
    }

    #[test]
    fn a_press_outside_the_open_list_closes_it_and_picks_nothing() {
        let mut strip = strip();
        let pressed = middle(RATE_LEFT, 1.0);
        let ContentPoint { x, .. } = pressed;

        assert_eq!(strip.press(pressed), None);
        assert_eq!(strip.press(ContentPoint::new(x - 100.0, row_y(0))), None);
        assert!(!strip.is_listing());

        assert_eq!(strip.press(pressed), None);
        assert_eq!(strip.press(ContentPoint::new(x, row_y(3))), None);
        assert!(!strip.is_listing());

        assert_eq!(strip.press(pressed), None);
        assert_eq!(strip.press(ContentPoint::new(x, f32::from(HEIGHT))), None);
    }

    #[test]
    fn a_press_on_another_box_opens_its_list_and_on_the_same_box_closes_it() {
        let mut strip = strip();
        let buffer = middle(BUFFER_LEFT, 0.0);

        assert_eq!(strip.press(middle(RATE_LEFT, 0.0)), None);
        assert_eq!(strip.press(buffer), None);
        assert!(strip.is_listing());
        assert_eq!(
            strip.press(ContentPoint::new(buffer.x, row_y(0))),
            Some(Change::Buffer(Dir::Input, None))
        );

        assert_eq!(strip.press(buffer), None);
        assert_eq!(strip.press(buffer), None);
        assert!(!strip.is_listing());
    }

    #[test]
    fn the_wheel_scrolls_a_long_list_and_rows_follow_it() {
        let mut strip = strip();
        let names: Vec<String> = (0..12).map(|index| format!("device {index}")).collect();
        let device = middle(DEVICE_LEFT, 0.0);

        strip.relist(PerDir::from_fn(|_| names.clone()));
        assert_eq!(strip.press(device), None);
        assert_eq!(strip.wheel(device, -3), None);
        assert_eq!(strip.wheel(device, -100), None);
        assert_eq!(
            strip.press(ContentPoint::new(device.x, row_y(0))),
            Some(Change::Input(Some(Pick::Device("device 4".to_owned()))))
        );
    }

    #[test]
    fn a_new_listing_shows_in_the_open_device_list_only() {
        let mut strip = strip();
        let device = middle(DEVICE_LEFT, 0.0);

        assert!(!strip.relist(named(&["old"])));
        assert_eq!(strip.press(device), None);
        assert!(strip.relist(named(&["new"])));
        assert!(!strip.relist(named(&["new"])));
        assert_eq!(
            strip.press(ContentPoint::new(device.x, row_y(2))),
            Some(Change::Input(Some(Pick::Device("new".to_owned()))))
        );

        assert_eq!(strip.press(middle(RATE_LEFT, 0.0)), None);
        assert!(!strip.relist(named(&["newer"])));
    }

    #[test]
    fn a_device_list_tells_apart_long_names_that_differ_at_the_end() {
        let mut strip = strip();
        let long = "A sound card whose name runs far past its box, port";

        strip.relist(named(&[&format!("{long} 1"), &format!("{long} 2")]));
        assert_eq!(strip.press(middle(DEVICE_LEFT, 0.0)), None);

        let list = strip.open.as_ref().unwrap();

        assert_eq!(list.labels[2..], [format!("{long} 1"), format!("{long} 2")]);
    }

    #[test]
    fn a_list_too_wide_for_the_screen_stays_on_it() {
        let mut strip = strip();
        let endless = "port ".repeat(200);

        strip.relist(named(&[&endless]));
        assert_eq!(strip.press(middle(DEVICE_LEFT, 1.0)), None);

        let list = strip.open.as_ref().unwrap();
        let area = list.area();

        assert!(area.x() + area.width() <= host::SCREEN.0 as f32);
        assert!(list.labels[1].ends_with("..."));
    }

    #[test]
    fn closing_the_list_picks_nothing_and_closes_it_once() {
        let mut strip = strip();
        let io = strip.io.clone();

        assert_eq!(strip.press(middle(RATE_LEFT, 0.0)), None);
        assert!(strip.close_list());
        assert!(!strip.is_listing());
        assert!(!strip.close_list());
        assert_eq!(strip.io, io);
    }

    #[test]
    fn a_press_on_the_tempo_drags_it_up_and_down_within_its_range() {
        let mut strip = strip();
        let pressed = middle(Field::Bpm.shape(2).left, 0.0);
        let at = |y| ContentPoint::new(pressed.x, y);

        assert_eq!(strip.press(pressed), Some(Change::Bpm(MIN_TEMPO)));
        assert!(strip.is_dragging());
        assert_eq!(
            strip.drag(at(pressed.y - 30.0)),
            Some(Change::Bpm(MIN_TEMPO + 10))
        );
        assert_eq!(
            strip.drag(at(pressed.y + 300.0)),
            Some(Change::Bpm(MIN_TEMPO))
        );
        assert_eq!(
            strip.drag(at(pressed.y - 3_000.0)),
            Some(Change::Bpm(MAX_TEMPO))
        );

        strip.release();
        assert!(!strip.is_dragging());
        assert_eq!(strip.drag(pressed), None);
    }

    #[test]
    fn the_tempo_steps_with_its_plus_and_minus() {
        let mut strip = strip();
        let Shape { left, width, .. } = Field::Bpm.shape(2);
        let y = middle(left, 0.0).y;
        let (plus, minus) = (
            ContentPoint::new(left + width - 10.0, y),
            ContentPoint::new(left + width - 40.0, y),
        );

        assert_eq!(strip.press(plus), Some(Change::Bpm(MIN_TEMPO + 1)));
        assert!(!strip.is_dragging());
        strip.show_tempo(f32::from(MIN_TEMPO + 1));
        assert_eq!(strip.press(minus), Some(Change::Bpm(MIN_TEMPO)));
        strip.show_tempo(f32::from(MIN_TEMPO));
        assert_eq!(strip.press(minus), Some(Change::Bpm(MIN_TEMPO)));
    }

    #[test]
    fn the_wheel_up_steps_a_box_to_its_previous_entry_and_raises_the_tempo() {
        let mut strip = strip();
        let (rate, scale, tempo) = (
            middle(RATE_LEFT, 0.0),
            middle(Field::Scale.shape(2).left, 0.0),
            middle(Field::Bpm.shape(2).left, 0.0),
        );

        assert_eq!(
            strip.wheel(rate, -1),
            Some(Change::Rate(Dir::Input, 96_000))
        );
        assert_eq!(strip.wheel(rate, 1), Some(Change::Rate(Dir::Input, 44_100)));
        assert_eq!(
            strip.wheel(rate, -5),
            Some(Change::Rate(Dir::Input, 96_000))
        );
        assert!(strip.apply(&Change::Rate(Dir::Input, 96_000)));
        assert_eq!(strip.wheel(rate, -1), None);

        assert_eq!(strip.wheel(scale, -1), Some(Change::Scale(1)));
        assert_eq!(strip.wheel(scale, 1), None);
        assert_eq!(
            strip.wheel(middle(BUFFER_LEFT, 1.0), -1),
            Some(Change::Buffer(Dir::Output, Some(64)))
        );
        assert_eq!(strip.wheel(middle(DEVICE_LEFT, 0.0), 1), None);

        assert_eq!(strip.wheel(tempo, 1), Some(Change::Bpm(MIN_TEMPO + 1)));
        assert_eq!(strip.wheel(tempo, -1), None);
        assert!(!strip.is_listing());
    }
}
