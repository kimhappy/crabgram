use core::num::NonZeroU32;

use crate::model::{getters, setters};

/// `dgcs:alignment`: where an image sits in its area.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Alignment {
    /// `dgcs:alignTopLeft`.
    TopLeft,
    /// `dgcs:alignTopMid`.
    TopMid,
    /// `dgcs:alignTopRight`.
    TopRight,
    /// `dgcs:alignLeftMid`.
    LeftMid,
    /// `dgcs:alignCenter`.
    Center,
    /// `dgcs:alignRightMid`.
    RightMid,
    /// `dgcs:alignBottomLeft`.
    BottomLeft,
    /// `dgcs:alignBottomMid`.
    BottomMid,
    /// `dgcs:alignBottomRight`.
    BottomRight,
}

impl Alignment {
    pub(crate) const fn curie(self) -> &'static str {
        match self {
            Self::TopLeft => "dgcs:alignTopLeft",
            Self::TopMid => "dgcs:alignTopMid",
            Self::TopRight => "dgcs:alignTopRight",
            Self::LeftMid => "dgcs:alignLeftMid",
            Self::Center => "dgcs:alignCenter",
            Self::RightMid => "dgcs:alignRightMid",
            Self::BottomLeft => "dgcs:alignBottomLeft",
            Self::BottomMid => "dgcs:alignBottomMid",
            Self::BottomRight => "dgcs:alignBottomRight",
        }
    }
}

/// `dgcs:Image`: an 8-bit RGBA PNG with a placement.
///
/// ```
/// use crabgram_styling::{Alignment, Image};
///
/// let dots = Image::new("asset/dots.png")
///     .with_alignment(Alignment::BottomMid)
///     .with_offset(0, -4);
///
/// assert_eq!(dots.offset(), (0, -4));
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Image {
    pub(crate) path: String,
    pub(crate) alignment: Option<Alignment>,
    pub(crate) offset: (i32, i32),
}

impl Image {
    /// Uses the default placement.
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            alignment: None,
            offset: (0, 0),
        }
    }

    getters! {
        /// PNG path relative to the Turtle file ([`PathReason`](crate::PathReason)).
        const path -> &str = |it| it.path.as_str();

        /// `None` leaves the default placement.
        const alignment -> Option<Alignment> = |it| it.alignment;

        /// `dgcs:offsetX` and `dgcs:offsetY`: right and down.
        const offset -> (i32, i32) = |it| it.offset;
    }

    setters! {
        with_alignment(alignment: Alignment) => alignment = Some(alignment);

        /// Moves the image `x` right and `y` down.
        with_offset(x: i32, y: i32) => offset = (x, y);
    }
}

impl From<&str> for Image {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl From<String> for Image {
    fn from(path: String) -> Self {
        Self::new(path)
    }
}

/// Direction in which frames run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Axis {
    /// Left to right.
    Horizontal,
    /// Top to bottom.
    Vertical,
}

impl Axis {
    /// How frames running this way sit, as a phrase.
    #[must_use]
    pub(crate) const fn arrangement(self) -> &'static str {
        match self {
            Self::Horizontal => "side by side",
            Self::Vertical => "one above the other",
        }
    }

    /// The dimension of a frame along this axis: `wide` or `high`.
    #[must_use]
    pub(crate) const fn extent(self) -> &'static str {
        match self {
            Self::Horizontal => "wide",
            Self::Vertical => "high",
        }
    }
}

/// Size of one frame of a strip along its [`Axis`]: `dgcs:width` for frames side by side,
/// `dgcs:height` for frames one above the other.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frame {
    pub(crate) axis: Axis,
    pub(crate) size: u32,
}

impl Frame {
    getters! {
        /// Direction the frames run.
        const axis -> Axis = |it| it.axis;

        /// Width of a frame running [`Axis::Horizontal`], height running [`Axis::Vertical`].
        const size -> u32 = |it| it.size;
    }

    /// Frames `size` wide side by side, `dgcs:width`.
    #[must_use]
    pub const fn horizontal(size: NonZeroU32) -> Self {
        Self {
            axis: Axis::Horizontal,
            size: size.get(),
        }
    }

    /// Frames `size` high one above the other, `dgcs:height`.
    #[must_use]
    pub const fn vertical(size: NonZeroU32) -> Self {
        Self {
            axis: Axis::Vertical,
            size: size.get(),
        }
    }

    /// Size of one frame cut from an image of `image`: the other dimension is the image's.
    #[must_use]
    pub const fn within(self, (width, height): (u32, u32)) -> (u32, u32) {
        match self.axis {
            Axis::Horizontal => (self.size, height),
            Axis::Vertical => (width, self.size),
        }
    }
}

/// Animated block image: `dgcs:BlockImageBypass` or `dgcs:BlockImageParameter`.
///
/// Frames run minimum to maximum for a parameter, on then off for bypass.
///
/// ```
/// use std::num::NonZeroU32;
///
/// use crabgram_styling::{Frame, Sprite};
///
/// let frame = Frame::horizontal(NonZeroU32::new(60).unwrap());
/// let lfo = Sprite::new("asset/block-lfo.png", frame).with_position(70, 50);
///
/// assert_eq!((lfo.position(), lfo.frame().size()), ((70, 50), 60));
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sprite {
    pub(crate) path: String,
    pub(crate) position: (u32, u32),
    pub(crate) frame: Frame,
}

impl Sprite {
    /// Frames of `frame` at the top left corner.
    #[must_use]
    pub fn new(path: impl Into<String>, frame: Frame) -> Self {
        Self {
            path: path.into(),
            position: (0, 0),
            frame,
        }
    }

    getters! {
        /// PNG path relative to the Turtle file.
        const path -> &str = |it| it.path.as_str();

        /// Left and top edge in the block image, `dgcs:x` and `dgcs:y`.
        const position -> (u32, u32) = |it| it.position;

        /// Size of one frame.
        const frame -> Frame = |it| it.frame;
    }

    setters! {
        /// Puts the top left corner at `x`, `y`.
        with_position(x: u32, y: u32) => position = (x, y);
    }
}
