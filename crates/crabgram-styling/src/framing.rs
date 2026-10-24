use core::{fmt, iter};
use std::collections::{BTreeMap, BTreeSet};

use crabgram::Control;

use crate::{
    Animation, Axis, BlockImage, BlockSettings, Button, Bypass, Frame, Image, Pagination, Styling,
    TopBar, Widget, host,
    model::{WidgetSource, getters},
};

/// How an image is cut into frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Framing {
    /// Any image.
    Free,
    /// Exactly these frames.
    #[non_exhaustive]
    Exact {
        /// The frames.
        cut: Cut,
    },
    /// This many equal frames that split the image along an axis.
    #[non_exhaustive]
    Split {
        /// The axis the frames run along; `None` for the long side of the image.
        axis: Option<Axis>,
        /// How many frames there are.
        frames: u32,
    },
    /// Frames in a row whose size and number follow from the image.
    #[non_exhaustive]
    Strip {
        /// How big one frame is.
        frame: FrameSize,
        /// How many frames there are.
        frames: FrameCount,
    },
}

/// The size of one frame of a [`Framing::Strip`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FrameSize {
    /// Square frames up to this size, along the long side of the image.
    Square(u32),
    /// Frames of this size along its axis.
    Along(Frame),
    /// Frames of exactly this size, side by side.
    Fixed((u32, u32)),
}

/// The number of frames of a [`Framing::Strip`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FrameCount {
    /// Any number.
    Any,
    /// Exactly this many.
    Exactly(u32),
    /// This many or more.
    AtLeast(u32),
    /// One up to this many.
    AtMost(u32),
}

impl FrameCount {
    /// One frame per value of a control with `values`, at least two for `None`.
    pub(crate) const fn of(values: Option<u32>) -> Self {
        match values {
            Some(values) => Self::Exactly(values),
            None => Self::AtLeast(2),
        }
    }

    const fn admits(self, frames: u32) -> bool {
        match self {
            Self::Any => true,
            Self::Exactly(count) => frames == count,
            Self::AtLeast(count) => frames >= count,
            Self::AtMost(count) => frames <= count,
        }
    }
}

impl fmt::Display for FrameCount {
    /// The number before `frames`, as in "at least 2".
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => write!(formatter, "any number of"),
            Self::Exactly(count) => write!(formatter, "{count}"),
            Self::AtLeast(count) => write!(formatter, "at least {count}"),
            Self::AtMost(count) => write!(formatter, "1 to {count}"),
        }
    }
}

/// Frames an image is cut into.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Cut {
    frame: (u32, u32),
    frames: u32,
    axis: Axis,
}

const fn length(axis: Axis, (width, height): (u32, u32)) -> u32 {
    match axis {
        Axis::Horizontal => width,
        Axis::Vertical => height,
    }
}

const fn long_axis((width, height): (u32, u32)) -> Axis {
    if width >= height {
        Axis::Horizontal
    } else {
        Axis::Vertical
    }
}

impl Cut {
    getters! {
        /// Size of one frame.
        const frame -> (u32, u32) = |it| it.frame;

        /// Number of frames.
        const frames -> u32 = |it| it.frames;

        /// Direction the frames run.
        const axis -> Axis = |it| it.axis;
    }

    /// As many `frame`s as fit along their axis in an image of `size`, at least one.
    fn of(frame: Frame, size: (u32, u32)) -> Self {
        let frames = length(frame.axis, size) / frame.size.max(1);

        Self {
            frame: frame.within(size),
            frames: frames.max(1),
            axis: frame.axis,
        }
    }

    /// `frames` equal frames along `axis` in an image of `size`.
    fn split(axis: Axis, frames: u32, size: (u32, u32)) -> Self {
        Self::of(
            Frame {
                axis,
                size: length(axis, size) / frames.max(1),
            },
            size,
        )
    }

    /// Size of the image the frames tile.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        let (width, height) = self.frame;

        match self.axis {
            Axis::Horizontal => (width.saturating_mul(self.frames), height),
            Axis::Vertical => (width, height.saturating_mul(self.frames)),
        }
    }

    /// Top left corner of frame `index`; an index past the end gives the last frame.
    #[must_use]
    pub fn offset(&self, index: u32) -> (u32, u32) {
        let index = index.min(self.frames.saturating_sub(1));

        match self.axis {
            Axis::Horizontal => (self.frame.0.saturating_mul(index), 0),
            Axis::Vertical => (0, self.frame.1.saturating_mul(index)),
        }
    }
}

/// `frames` after `count`, singular for exactly one.
const fn noun(count: FrameCount) -> &'static str {
    match count {
        FrameCount::Exactly(1) => "frame",
        _ => "frames",
    }
}

impl fmt::Display for Cut {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (width, height) = self.frame;
        let count = FrameCount::Exactly(self.frames);

        write!(formatter, "{count} {} of {width}x{height}", noun(count))
    }
}

const BYPASS_SWITCH: Cut = Cut {
    frame: host::BYPASS,
    frames: 2,
    axis: Axis::Horizontal,
};

impl Framing {
    /// One frame of `(width, height)`.
    #[must_use]
    pub const fn image(frame: (u32, u32)) -> Self {
        Self::Exact {
            cut: Cut {
                frame,
                frames: 1,
                axis: Axis::Horizontal,
            },
        }
    }

    /// This many frames, side by side.
    #[must_use]
    pub const fn row(frames: u32) -> Self {
        Self::Split {
            axis: Some(Axis::Horizontal),
            frames,
        }
    }

    /// This many frames along the long side of the image.
    #[must_use]
    pub const fn along_long_side(frames: u32) -> Self {
        Self::Split { axis: None, frames }
    }

    /// One screen-sized frame per page, side by side.
    #[must_use]
    pub const fn pages(pages: u32) -> Self {
        Self::Exact {
            cut: Cut {
                frame: host::SCREEN,
                frames: pages,
                axis: Axis::Horizontal,
            },
        }
    }

    /// Two frames of `frame`, on then off.
    #[must_use]
    pub const fn switch(frame: Frame) -> Self {
        Self::Strip {
            frame: FrameSize::Along(frame),
            frames: FrameCount::Exactly(2),
        }
    }

    /// Square control frames up to [`host::CONTROL`], `frames` of them.
    #[must_use]
    pub const fn control(frames: FrameCount) -> Self {
        Self::Strip {
            frame: FrameSize::Square(host::CONTROL),
            frames,
        }
    }

    /// How an image of `size` is cut into frames.
    ///
    /// ```
    /// use crabgram_styling::{Axis, FrameCount, Framing};
    ///
    /// let cut = Framing::control(FrameCount::AtLeast(2)).cut((145, 14645));
    ///
    /// assert_eq!(
    ///     (cut.frame(), cut.frames(), cut.axis()),
    ///     ((145, 145), 101, Axis::Vertical)
    /// );
    /// ```
    #[must_use]
    pub fn cut(self, size: (u32, u32)) -> Cut {
        match self {
            Self::Free => Cut::of(
                Frame {
                    axis: Axis::Horizontal,
                    size: size.0,
                },
                size,
            ),
            Self::Exact { cut } => cut,
            Self::Split { axis, frames } => {
                Cut::split(axis.unwrap_or_else(|| long_axis(size)), frames, size)
            }
            Self::Strip { frame, .. } => match frame {
                FrameSize::Square(_) => Cut::of(
                    Frame {
                        axis: long_axis(size),
                        size: size.0.min(size.1),
                    },
                    size,
                ),
                FrameSize::Along(frame) => Cut::of(frame, size),
                FrameSize::Fixed((width, _)) => Cut::of(
                    Frame {
                        axis: Axis::Horizontal,
                        size: width,
                    },
                    size,
                ),
            },
        }
    }

    /// Whether an image of `size` fits this framing.
    ///
    /// ```
    /// use crabgram_styling::{FrameCount, Framing};
    ///
    /// let knob = Framing::control(FrameCount::Exactly(101));
    ///
    /// assert!(knob.admits((145, 14645)));
    /// assert!(!Framing::control(FrameCount::AtLeast(2)).admits((145, 200)));
    /// ```
    #[must_use]
    pub fn admits(self, size: (u32, u32)) -> bool {
        let cut = self.cut(size);
        let tiled = cut.size() == size;

        match self {
            Self::Free => true,
            Self::Exact { .. } | Self::Split { .. } => tiled,
            Self::Strip { frame, frames } => {
                let sized = match frame {
                    FrameSize::Along(_) => true,
                    FrameSize::Square(largest) => cut.frame.0 <= largest,
                    FrameSize::Fixed(fixed) => cut.frame == fixed,
                };

                tiled && sized && frames.admits(cut.frames)
            }
        }
    }
}

/// Why an image of [`size`](Self::size()) breaks a [`framing`](Self::framing()), from
/// [`Framing::mismatch`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Mismatch {
    framing: Framing,
    size: (u32, u32),
}

impl Mismatch {
    getters! {
        /// The framing the image breaks.
        const framing -> Framing = |it| it.framing;

        /// Width and height of the image.
        const size -> (u32, u32) = |it| it.size;
    }
}

impl fmt::Display for Mismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (width, height) = self.size;
        let cut = self.framing.cut(self.size);

        write!(formatter, "{width}x{height}")?;

        if matches!(self.framing, Framing::Split { .. } | Framing::Strip { .. }) {
            if cut.size() == self.size {
                write!(formatter, ", cut into {cut}")?;
            } else {
                let (frame_width, frame_height) = cut.frame;

                write!(
                    formatter,
                    ", which frames of {frame_width}x{frame_height} do not tile"
                )?;
            }
        }

        write!(formatter, ", but it needs {}", self.framing)
    }
}

impl Framing {
    /// Why an image of `size` breaks this framing, or `None` when it fits.
    ///
    /// ```
    /// use crabgram_styling::Framing;
    ///
    /// let bar = Framing::image((800, 60));
    ///
    /// assert_eq!(
    ///     bar.mismatch((800, 50)).unwrap().to_string(),
    ///     "800x50, but it needs a 800x60 image"
    /// );
    /// assert!(bar.mismatch((800, 60)).is_none());
    /// ```
    #[must_use]
    pub fn mismatch(self, size: (u32, u32)) -> Option<Mismatch> {
        (!self.admits(size)).then_some(Mismatch {
            framing: self,
            size,
        })
    }
}

impl Button {
    /// Exactly two [`frame`](Self::frame)s running along [`axis`](Self::axis).
    ///
    /// ```
    /// use crabgram_styling::{Button, Framing};
    ///
    /// assert_eq!(Button::More.framing().cut((100, 50)).frame(), (50, 50));
    /// ```
    #[must_use]
    pub const fn framing(self) -> Framing {
        Framing::Exact {
            cut: Cut {
                frame: self.frame(),
                frames: 2,
                axis: self.axis(),
            },
        }
    }
}

impl Bypass {
    /// Switch image path and its framing: two [`host::BYPASS`] frames side by side.
    #[must_use]
    pub fn switch_framing(&self) -> Option<(&str, Framing)> {
        self.control
            .as_deref()
            .map(|path| (path, Framing::Exact { cut: BYPASS_SWITCH }))
    }
}

impl fmt::Display for Framing {
    /// Lower case, without a full stop, to end a sentence.
    ///
    /// ```
    /// use crabgram_styling::{FrameCount, Framing};
    ///
    /// assert_eq!(
    ///     Framing::control(FrameCount::AtLeast(2)).to_string(),
    ///     "at least 2 square frames up to 145"
    /// );
    /// ```
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Free => write!(formatter, "any image"),
            Self::Exact {
                cut:
                    Cut {
                        frame: (width, height),
                        frames: 1,
                        ..
                    },
            } => write!(formatter, "a {width}x{height} image"),
            Self::Exact { cut } => write!(formatter, "{cut}, {}", cut.axis.arrangement()),
            Self::Split { axis, frames } => {
                let count = FrameCount::Exactly(frames);
                let noun = noun(count);

                match axis {
                    None => write!(formatter, "{count} {noun} along its long side"),
                    Some(axis) => write!(formatter, "{count} {noun} {}", axis.arrangement()),
                }
            }
            Self::Strip { frame, frames } => {
                let noun = noun(frames);

                match frame {
                    FrameSize::Square(largest) => {
                        write!(formatter, "{frames} square {noun} up to {largest}")
                    }
                    FrameSize::Along(frame) => write!(
                        formatter,
                        "{frames} {noun} {} {}, {}",
                        frame.size,
                        frame.axis.extent(),
                        frame.axis.arrangement()
                    ),
                    FrameSize::Fixed((width, height)) => {
                        write!(
                            formatter,
                            "{frames} {noun} of {width}x{height}, side by side"
                        )
                    }
                }
            }
        }
    }
}

type Framed<'a> = (&'a str, Framing);

const BLOCK: (u32, u32) = (host::BLOCK_IMAGE, host::BLOCK_IMAGE);

fn framed(framing: Framing) -> impl for<'a> Fn(&'a Image) -> Framed<'a> {
    move |image| (image.path.as_str(), framing)
}

fn block_framings<'a>(
    block: &'a BlockImage,
    controls: &[Control],
) -> impl Iterator<Item = Framed<'a>> {
    let sprites = block.animations.iter().map(|Animation { symbol, sprite }| {
        let frames = controls
            .iter()
            .find(|it| it.symbol() == symbol)
            .and_then(Control::value_count);
        (
            sprite.path.as_str(),
            Framing::Strip {
                frame: FrameSize::Along(sprite.frame),
                frames: frames.map_or(FrameCount::Any, FrameCount::Exactly),
            },
        )
    });

    let block_image = Framing::Strip {
        frame: FrameSize::Fixed(BLOCK),
        frames: FrameCount::AtMost(2),
    };

    iter::once((block.path.as_str(), block_image))
        .chain(
            block
                .bypass
                .iter()
                .map(|bypass| (bypass.path.as_str(), Framing::switch(bypass.frame))),
        )
        .chain(sprites)
}

fn top_bar_framings(bar: &TopBar) -> impl Iterator<Item = Framed<'_>> {
    let images = [
        bar.background
            .as_ref()
            .map(framed(Framing::image(host::TOP_BAR))),
        bar.block_name
            .as_ref()
            .map(framed(Framing::image(host::BLOCK_NAME))),
    ]
    .into_iter()
    .flatten();
    let buttons = bar
        .buttons
        .iter()
        .map(|(button, path)| (path.as_str(), button.framing()));
    let scenes = bar
        .scene_control
        .iter()
        .flat_map(|scenes| [&scenes.active_scene, &scenes.all_scenes])
        .map(|image| (image.path.as_str(), Framing::image(host::SCENE_CONTROL)));

    images.chain(buttons).chain(scenes)
}

fn bypass_framings(bypass: &Bypass) -> impl Iterator<Item = Framed<'_>> {
    [
        bypass
            .background
            .as_ref()
            .map(framed(Framing::image(host::BYPASS))),
        bypass
            .background_scenes
            .as_ref()
            .map(framed(Framing::image(host::BYPASS))),
        bypass.switch_framing(),
    ]
    .into_iter()
    .flatten()
}

/// The source of the widget each visible control shows, with the control's value count.
fn shown(settings: &BlockSettings, controls: &[Control]) -> Vec<(WidgetSource, Option<u32>)> {
    controls
        .iter()
        .filter(|control| !control.is_hidden())
        .filter_map(|control| {
            let (source, _) = settings.sourced_widget_of(control)?;

            Some((source, control.value_count()))
        })
        .collect()
}

/// Value counts of the visible controls showing the widget of `source`, or any count when none
/// does.
fn frame_counts(
    source: WidgetSource,
    shown: &[(WidgetSource, Option<u32>)],
) -> BTreeSet<Option<u32>> {
    let counts: BTreeSet<_> = shown
        .iter()
        .filter(|(it, _)| *it == source)
        .map(|(_, count)| *count)
        .collect();

    if counts.is_empty() {
        BTreeSet::from([None])
    } else {
        counts
    }
}

/// Lists the control image once per value count of the controls showing `widget`, from `source`.
fn widget_framings<'a>(
    source: WidgetSource,
    widget: &'a Widget,
    shown: &[(WidgetSource, Option<u32>)],
) -> impl Iterator<Item = Framed<'a>> + use<'a> {
    let backgrounds = [&widget.background, &widget.background_scenes]
        .into_iter()
        .filter_map(Option::as_ref)
        .map(framed(Framing::image(host::WIDGET)));
    let control = frame_counts(source, shown).into_iter().map(|count| {
        (
            widget.control.path.as_str(),
            Framing::control(FrameCount::of(count)),
        )
    });
    let overlays = widget
        .overlays
        .paths()
        .map(|(_, path)| (path, Framing::image(host::WIDGET)));

    backgrounds.chain(control).chain(overlays)
}

fn settings_framings<'a, 'c>(
    settings: &'a BlockSettings,
    controls: &'c [Control],
) -> impl Iterator<Item = Framed<'a>> + use<'a, 'c> {
    let pages = settings.pages(controls);
    let screen = [
        settings
            .background
            .as_ref()
            .map(framed(Framing::pages(pages))),
        settings
            .about
            .as_ref()
            .map(framed(Framing::image(host::ABOUT))),
        settings
            .pagination
            .as_ref()
            .map(|pagination| match pagination {
                Pagination::Dots(image) => (image.path.as_str(), Framing::row(pages)),
                Pagination::Button(image) => (image.path.as_str(), Framing::along_long_side(pages)),
            }),
    ]
    .into_iter()
    .flatten();
    let bar = settings.top_bar.iter().flat_map(top_bar_framings);
    let bypass = settings.bypass.iter().flat_map(bypass_framings);
    let shown = shown(settings, controls);
    let widgets = settings
        .sourced_widgets()
        .flat_map(move |(source, widget)| widget_framings(source, widget, &shown));

    screen.chain(bar).chain(bypass).chain(widgets)
}

impl Styling {
    /// Every named image in naming order, repeats included.
    #[must_use]
    pub(crate) fn framings(&self, controls: &[Control]) -> Vec<(&str, Framing)> {
        let icons = self
            .icons
            .iter()
            .flat_map(|it| [it.off.as_str(), it.on.as_str()])
            .map(|path| (path, Framing::image(BLOCK)));
        let block = self
            .block_image
            .iter()
            .flat_map(|block| block_framings(block, controls));
        let settings = self
            .block_settings
            .iter()
            .flat_map(|settings| settings_framings(settings, controls));

        icons.chain(block).chain(settings).collect()
    }

    /// Every named image once, with the framings it must fit in naming order, none twice.
    ///
    /// ```
    /// use crabgram::Controls;
    /// use crabgram_styling::{BlockSettings, FrameCount, Framing, Parameter, Styling, Widget};
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     gain: f32,
    ///     level: f32,
    /// }
    ///
    /// let knob = || Widget::new("asset/knob.png");
    /// let styling = Styling::default().with_block_settings(
    ///     BlockSettings::default()
    ///         .with_knob(knob())
    ///         .with_parameter(Parameter::of(Params::LEVEL).with_widget(knob())),
    /// );
    ///
    /// assert_eq!(
    ///     styling.images(Params::CONTROLS),
    ///     [(
    ///         "asset/knob.png",
    ///         vec![Framing::control(FrameCount::AtLeast(2))]
    ///     )]
    ///     .into()
    /// );
    /// ```
    #[must_use]
    pub fn images(&self, controls: &[Control]) -> BTreeMap<&str, Vec<Framing>> {
        self.framings(controls)
            .into_iter()
            .fold(BTreeMap::new(), |mut images, (path, framing)| {
                let framings: &mut Vec<_> = images.entry(path).or_default();

                if !framings.contains(&framing) {
                    framings.push(framing);
                }

                images
            })
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use crabgram::Controls;

    use super::{FrameCount, FrameSize, Framing};
    use crate::{BlockSettings, Frame, Pagination, Parameter, Styling, Widget, host};

    fn size(size: u32) -> NonZeroU32 {
        NonZeroU32::new(size).unwrap()
    }

    #[test]
    fn a_hidden_control_puts_no_frame_count_on_its_widget() {
        #[derive(Controls)]
        struct Params {
            gain: f32,
            #[control(hidden, min = 0, max = 4)]
            steps: u32,
        }

        let styling = Styling {
            block_settings: Some(BlockSettings::default().with_knob(Widget::new("asset/knob.png"))),
            ..Styling::default()
        };

        assert_eq!(
            styling.framings(Params::CONTROLS),
            [("asset/knob.png", Framing::control(FrameCount::AtLeast(2)))]
        );
    }

    #[test]
    fn the_images_group_the_framings_of_a_path_without_repeats() {
        #[derive(Controls)]
        struct Params {
            gain: f32,
            #[control(min = 0, max = 4)]
            steps: u32,
        }

        let knob = || Widget::new("asset/knob.png");
        let styling = Styling::default().with_block_settings(
            BlockSettings::default()
                .with_pagination(Pagination::Dots("asset/dots.png".into()))
                .with_knob(knob())
                .with_parameter(Parameter::of(Params::STEPS).with_widget(knob())),
        );
        let steps = Framing::control(FrameCount::Exactly(5));

        assert_eq!(
            styling.images(Params::CONTROLS),
            [
                ("asset/dots.png", vec![Framing::row(1)]),
                (
                    "asset/knob.png",
                    vec![Framing::control(FrameCount::AtLeast(2)), steps]
                ),
            ]
            .into()
        );
    }

    #[test]
    fn the_about_page_fills_the_screen_below_the_top_bar() {
        let about = Framing::image(host::ABOUT);

        assert!(about.admits((1424, 220)));
        assert!(!about.admits((1424, 900)));
    }

    #[test]
    fn exact_frames_admit_only_their_own_size() {
        let close = crate::Button::Close.framing();

        assert!(close.admits((85, 100)));
        assert!(!close.admits((85, 120)));
        assert!(!close.admits((170, 50)));
        assert_eq!(close.cut((85, 120)).frames, 2);
        assert_eq!(close.to_string(), "2 frames of 85x50, one above the other");
    }

    #[test]
    fn the_bypass_switch_is_two_frames_as_high_as_its_slot() {
        let (_, switch) = crate::Bypass::default()
            .with_control("asset/bypass.png")
            .switch_framing()
            .unwrap();
        let (width, height) = host::BYPASS;

        assert!(switch.admits((2 * width, height)));
        assert!(!switch.admits((2 * width, height + 10)));
        assert!(!switch.admits((width, height)));
    }

    #[test]
    fn one_frame_is_counted_in_the_singular() {
        assert_eq!(Framing::row(1).to_string(), "1 frame side by side");
    }

    #[test]
    fn a_widget_lists_its_image_once_per_number_of_values() {
        #[derive(Controls)]
        struct Params {
            gain: f32,
            level: f32,
            #[control(min = 0, max = 4)]
            steps: u32,
        }

        let styling = Styling {
            block_settings: Some(BlockSettings::default().with_knob(Widget::new("asset/knob.png"))),
            ..Styling::default()
        };

        assert_eq!(
            styling.framings(Params::CONTROLS),
            [
                ("asset/knob.png", Framing::control(FrameCount::AtLeast(2))),
                ("asset/knob.png", Framing::control(FrameCount::Exactly(5))),
            ]
        );
    }

    #[test]
    fn a_strip_runs_along_the_side_its_frame_is_given_for() {
        let beside = Framing::switch(Frame::horizontal(size(64)));
        let above = Framing::switch(Frame::vertical(size(64)));

        assert!(beside.admits((128, 64)));
        assert!(!beside.admits((64, 128)));
        assert!(above.admits((64, 128)));
        assert!(!above.admits((128, 64)));
        assert!(
            Framing::Strip {
                frame: FrameSize::Along(Frame::vertical(size(64))),
                frames: FrameCount::Any,
            }
            .admits((64, 64))
        );
    }

    #[test]
    fn pagination_dots_run_side_by_side() {
        let dots = Framing::row(3);

        assert!(dots.admits((360, 20)));
        assert!(!dots.admits((100, 20)));
        assert!(!dots.admits((20, 360)));
    }

    #[test]
    fn a_pagination_button_runs_either_way() {
        let button = Framing::along_long_side(3);

        assert!(button.admits((180, 50)));
        assert!(button.admits((60, 150)));
        assert!(!button.admits((100, 50)));
    }

    #[test]
    fn a_two_frame_block_image_runs_side_by_side() {
        let side = host::BLOCK_IMAGE;
        let block = Framing::Strip {
            frame: FrameSize::Fixed((side, side)),
            frames: FrameCount::AtMost(2),
        };

        assert!(block.admits((side, side)));
        assert!(block.admits((2 * side, side)));
        assert!(!block.admits((side, 2 * side)));
        assert!(!block.admits((3 * side, side)));
    }

    #[test]
    fn the_overlays_are_widget_sized() {
        let overlays = crate::Overlays {
            blocked: Some("asset/blocked.png".into()),
            inactive: Some("asset/inactive.png".into()),
        };
        let styling = Styling {
            block_settings: Some(
                BlockSettings::default()
                    .with_knob(Widget::new("asset/knob.png").with_overlays(overlays)),
            ),
            ..Styling::default()
        };
        let framings = styling.framings(&[]);

        assert_eq!(
            framings[1..],
            [
                ("asset/blocked.png", Framing::image(host::WIDGET)),
                ("asset/inactive.png", Framing::image(host::WIDGET)),
            ]
        );
    }

    #[test]
    fn a_fixed_framing_names_only_what_it_needs() {
        let switch = crate::Bypass::default()
            .with_control("asset/bypass.png")
            .switch_framing()
            .unwrap()
            .1;

        assert_eq!(
            switch.mismatch((214, 80)).unwrap().to_string(),
            "214x80, but it needs 2 frames of 107x50, side by side"
        );
    }

    #[test]
    fn a_framing_cut_by_size_names_its_cut() {
        assert_eq!(
            Framing::switch(Frame::horizontal(size(107)))
                .mismatch((321, 50))
                .unwrap()
                .to_string(),
            "321x50, cut into 3 frames of 107x50, but it needs 2 frames 107 wide, \
             side by side"
        );
    }

    #[test]
    fn a_framing_cut_by_size_names_frames_that_do_not_tile() {
        let button = Framing::along_long_side(3);

        assert_eq!(
            button.mismatch((100, 50)).unwrap().to_string(),
            "100x50, which frames of 33x50 do not tile, but it needs 3 frames along its long side"
        );
    }

    #[test]
    fn the_offset_of_a_frame_far_past_the_end_stays_in_range() {
        let cut = super::Cut {
            frame: (u32::MAX / 2, 50),
            frames: u32::MAX,
            axis: crate::Axis::Horizontal,
        };

        assert_eq!(cut.offset(u32::MAX), (u32::MAX, 0));
    }

    #[test]
    fn control_frames_are_square_and_counted() {
        let seventeen = Framing::control(FrameCount::Exactly(17));

        assert!(seventeen.admits((145, 17 * 145)));
        assert!(!seventeen.admits((145, 16 * 145)));
        assert!(!Framing::control(FrameCount::AtLeast(2)).admits((145, 145)));
        assert!(!Framing::control(FrameCount::AtLeast(2)).admits((146, 2 * 146)));
        assert!(!Framing::control(FrameCount::AtLeast(2)).admits((100, 250)));
    }
}
