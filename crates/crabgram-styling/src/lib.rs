//! Typed Darkglass custom styling for crabgram plugins, written out as Turtle.
//!
//! [`Styling::turtle`] writes [`turtle::FILE`], [`BlockSettings::slots`] lays out the widgets,
//! [`Styling::problems`] finds mistakes in the styling, and [`Framing`] gives the image sizes.
//! Property names come from the `dgcs:` ontology.
//!
//! ```
//! use crabgram::{Choice, Controls};
//! use crabgram_styling::{BlockSettings, HostStyle, Parameter, Styling, Widget};
//!
//! #[derive(Choice)]
//! enum Shape {
//!     Sine,
//!     Square,
//!     Random,
//! }
//!
//! #[derive(Controls)]
//! struct Params {
//!     #[control(default = 3.0, min = 0.1, max = 12.0)]
//!     rate: f32,
//!     shape: Shape,
//! }
//!
//! let controls = Params::CONTROLS;
//! let styling = Styling::default().with_block_settings(
//!     BlockSettings::default()
//!         .with_knob(Widget::new("asset/knob.png"))
//!         .with_parameter(Parameter::of(Params::SHAPE).with_style(HostStyle::ListWith3Slots)),
//! );
//!
//! assert!(styling.problems(controls).is_empty());
//! assert_eq!(styling.block_settings().unwrap().pages(controls), 1);
//! assert!(
//!     styling
//!         .turtle("urn:example:tremolo")
//!         .unwrap()
//!         .contains("dgcs:listWith3Slots")
//! );
//! ```
//!
//! # Attaching styling
//!
//! Styling lives in its own Turtle file, listed in `manifest.ttl` (`crabgram-build` writes both):
//!
//! ```turtle
//! <urn:example:tremolo> a lv2:Plugin ; rdfs:seeAlso <tremolo.ttl> , <styling.ttl> .
//! ```
//!
//! `styling.ttl` holds `dgcs:blockImage` ([`BlockImage`]) and `dgcs:blockSettings`
//! ([`BlockSettings`]), both optional. [`Icons`] go to the plugin description instead.
//!
//! # Images and frames
//!
//! Images are 8-bit RGBA PNGs at plain paths relative to the Turtle file ([`PathReason`]).
//! `crabgram-build` reports a missing file and any other encoding.
//!
//! An animated image is a **strip** of equal frames, side by side or stacked. [`Framing`] holds the
//! rule for each image; `crabgram-build` checks it.
//!
//! | Image | Frames | Order |
//! | - | - | - |
//! | Parameter control | 2 or more | minimum first |
//! | Bypass control | 2 | on, off |
//! | Block image | 1, or 2 side by side | on, off |
//! | Top bar button | 2, along [`Button::axis`] | |
//! | Background, pagination dots, pagination button | one per page | page 1 first |
//!
//! Parameter frames are square, at most [`host::CONTROL`]. Counts: knob 2 or more, integer
//! `max − min + 1`, list one per label, toggle 2, meter one per step.
//!
//! Every [`Image`] takes an [`Alignment`] and an offset.
//!
//! # Widgets and slots
//!
//! [`crabgram::Control::widget`] gives the kind of widget a control shows; hidden controls take
//! no slot.
//!
//! A default [`Widget`] styles every parameter of its kind; a [`Parameter`] styles one port by
//! symbol and wins. A widget always has control frames, `dgcs:control`: [`Widget::new`] takes them.
//!
//! [`BlockSettings::slots`] lays out the widgets and [`BlockSettings::pages`] counts the pages.
//! See [`BlockSettings::start_padding`], [`Parameter::pin`] and [`HostStyle`] for padding,
//! pinning and multi-slot lists.
//!
//! # Animation
//!
//! An output control animates its [`Widget`], or a [`Sprite`] on the block
//! ([`BlockImage::with_animation`]), with the values `run` writes.

mod framing;
pub mod host;
mod image;
mod layout;
mod model;
mod problems;
pub mod turtle;
#[cfg(feature = "serde")]
pub mod ui;

pub use crate::{
    framing::{Cut, FrameCount, FrameSize, Framing, Mismatch},
    image::{Alignment, Axis, Frame, Image, Sprite},
    layout::Slot,
    model::{
        Animation, BlockImage, BlockSettings, Button, Bypass, HostStyle, Icons, Overlays,
        Pagination, Parameter, SceneControl, Styling, TopBar, Widget,
    },
    problems::{PathReason, Problem},
};
