//! Write LV2 plugins in Rust.
//!
//! # Setup
//!
//! crabgram needs nightly for `generic_const_exprs`. The project template sets it up; by hand:
//!
//! 1. Limit the next trait solver to coherence in `.cargo/config.toml`:
//!    ```toml
//!    [build]
//!    rustflags = ["-Znext-solver=coherence"]
//!    rustdocflags = ["-Znext-solver=coherence"]
//!    ```
//! 2. Build a `cdylib`, enable `testing` for tests, and allow the incomplete feature lint:
//!    ```toml
//!    [lib]
//!    crate-type = ["cdylib", "rlib"]
//!
//!    [dependencies]
//!    crabgram = { git = "https://github.com/kimhappy/crabgram" }
//!
//!    [dev-dependencies]
//!    crabgram = { git = "https://github.com/kimhappy/crabgram", features = ["testing"] }
//!
//!    [lints.rust]
//!    incomplete_features = "allow"
//!    ```
//! 3. A crate that names [`Counts::CHANNELS`] or [`Counts::PORTS`] of a plugin in a type, as the
//!    plugin itself does through [`Context::buffers`] or a test through `Rig` and `Block`, enables
//!    `#![feature(generic_const_exprs)]`. A generic helper over such a plugin `P` adds the bound
//!    `[(); P::CHANNELS]:` or `[(); P::PORTS]:` it names.
//!
//! # Features
//!
//! | Feature | Adds |
//! | - | - |
//! | `licensing` | The license check that [`Info::licensing`] asks for, linked in. |
//! | `serde` | `Serialize` and `Deserialize` on the control types; a deserialized [`Control`] is checked and leaks its strings. |
//! | `testing` | The `testing` module: `Rig`, `Block` and `CountingAllocator`. |
//!
//! # A plugin
//!
//! Implement [`Plugin`] and hand it to [`export!`]:
//! ```
//! # #![feature(generic_const_exprs)]
//! use crabgram::prelude::*;
//!
//! #[derive(Debug, PartialEq, Choice)]
//! pub enum Mode {
//!     Boost,
//!     Cut,
//! }
//!
//! #[derive(Controls)]
//! pub struct Params {
//!     /// How far to boost or cut, in decibels.
//!     #[control(default = 6.0, min = 0.0, max = 24.0, unit = Decibel, quick_pot)]
//!     pub amount: f32,
//!     /// Whether to boost or cut.
//!     pub mode: Mode,
//!     /// The output peak, shown as a meter.
//!     #[control(output, default = -60.0, min = -60.0, max = 0.0, unit = Decibel)]
//!     pub peak: f32,
//! }
//!
//! pub struct Gain {
//!     gain: f32,
//! }
//!
//! impl Plugin for Gain {
//!     type Controls = Params;
//!
//!     const INFO: Info = info! {
//!         uri: c"https://example.com/plugins/gain",
//!         name: "Example Gain",
//!         description: "Turns the level up or down.",
//!         abbreviation: "GN",
//!         category: Utility,
//!         channels: 2,
//!     };
//!
//!     fn new() -> Self {
//!         Self { gain: 1.0 }
//!     }
//!
//!     fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
//!         let decibels = match controls.mode {
//!             Mode::Boost => controls.amount,
//!             Mode::Cut => -controls.amount,
//!         };
//!         let target = if context.is_enabled() {
//!             10.0_f32.powf(decibels / 20.0)
//!         } else {
//!             1.0
//!         };
//!         let start = if context.is_reset() {
//!             target
//!         } else {
//!             self.gain
//!         };
//!         let step = (target - start) / FRAMES as f32;
//!
//!         self.gain = target;
//!
//!         let peak = context
//!             .buffers()
//!             .into_iter()
//!             .flat_map(|channel| channel.iter_mut().zip(1..))
//!             .fold(0.0_f32, |peak, (sample, frame)| {
//!                 *sample *= start + step * frame as f32;
//!                 peak.max(sample.abs())
//!             });
//!
//!         context.write(Params::PEAK, (20.0 * peak.max(1.0e-3).log10()).max(-60.0));
//!     }
//! }
//!
//! export!(Gain);
//! ```
//!
//! `crabgram-build` and `crabgram-styling` write the Turtle description and styling from the same
//! types.
//!
//! # Where to look
//!
//! - Controls: [`Controls`](derive@Controls), [`Choice`](derive@Choice), [`Unit`]
//! - Description: [`info!`](macro@info), [`Info`]
//! - Processing: [`Plugin::run`], [`Context`], [`anagram`]
//! - Control states and changes: [`Context::set_state`], [`Context::request_change`], [`Field`]
//! - Tests: `testing`
//! - Bundle, check, deploy: `crabgram-build`
#![feature(generic_const_exprs)]

pub mod anagram;
mod channels;
mod context;
mod control;
mod field;
mod info;
mod plugin;
pub mod port;
#[cfg(feature = "testing")]
pub mod testing;
mod text;

mod runtime;
pub use crabgram_macros::{Choice, Controls, info};
pub use runtime::lv2;

mod sealed {
    /// Implemented by `#[derive(Controls)]`.
    #[diagnostic::on_unimplemented(
        message = "only `#[derive(Controls)]` implements `Controls`, not one for `{Self}`",
        note = "derive `Controls` instead of implementing the trait"
    )]
    pub trait SealedControls {}

    /// Implemented by crabgram for the plain types and by `#[derive(Choice)]`.
    #[diagnostic::on_unimplemented(
        message = "`{Self}` is not a control type",
        note = "a control field is `f32`, `bool`, an integer up to 32 bits or an enum deriving `Choice`"
    )]
    pub trait SealedValue {}

    pub trait SealedAccess {}
}

/// Expansion target of the derive macros and `info!`.
#[doc(hidden)]
pub mod __private {
    pub use crate::{
        control::{Direction, Flags, make as control},
        field::make as field,
        info::make as info,
        sealed::{SealedControls, SealedValue},
        text::fits_iri,
    };
}

/// What a plugin crate imports: `use crabgram::prelude::*;`.
pub mod prelude {
    pub use crate::{
        Activity, Choice, Context, ControlState, Controls, Counts, Info, Plugin,
        anagram::{FRAMES, MAX_BPM, MIN_BPM, SAMPLE_RATE},
        export, info,
    };
}

pub use crate::{
    context::Context,
    control::{Choice, Control, ControlProblem, Controls, Kind, Unit, Value, WidgetKind},
    field::{Field, access},
    info::{Category, Info, InfoProblem, Licensing, Maintainer, Version},
    plugin::{Counts, Plugin},
    runtime::host::{Activity, ControlState, Refusal},
};

/// Defines the `lv2_descriptor` entry point of `$plugin`, once per `cdylib`.
///
/// ```
/// # #![feature(generic_const_exprs)]
/// use crabgram::prelude::*;
///
/// # #[derive(Controls)] struct Params { level: f32 }
/// struct Silence;
///
/// impl Plugin for Silence {
///     # type Controls = Params;
///     # const INFO: Info = info! {
///     #     uri: c"urn:example:silence",
///     #     name: "Silence",
///     #     description: "",
///     #     abbreviation: "SIL",
///     #     category: Utility,
///     #     channels: 1,
///     # };
///     # fn new() -> Self { Self }
///     # fn run(&mut self, _: &Params, context: &mut Context<'_, Self>) {
///     #     let [samples] = context.buffers();
///     #     samples.fill(0.0);
///     # }
/// }
///
/// export!(Silence);
///
/// let descriptor = lv2_descriptor(0);
/// assert!(!descriptor.is_null());
/// assert!(lv2_descriptor(1).is_null());
/// ```
#[macro_export]
macro_rules! export {
    ($plugin:ty) => {
        /// The descriptor of plugin `index`, or null past the last one.
        #[unsafe(no_mangle)]
        pub extern "C" fn lv2_descriptor(
            index: ::core::primitive::u32,
        ) -> *const $crate::lv2::Descriptor {
            static DESCRIPTOR: $crate::lv2::Descriptor = $crate::lv2::descriptor::<$plugin>();

            if index == 0 {
                &raw const DESCRIPTOR
            } else {
                ::core::ptr::null()
            }
        }
    };
}
