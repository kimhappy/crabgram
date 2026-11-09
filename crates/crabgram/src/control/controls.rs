use super::Control;
#[cfg(doc)]
use super::{Choice, Unit};

/// The control ports of a plugin: a struct with one field per port, in port order.
///
/// Derive it. The field type sets the port kind and `#[control(...)]` the rest:
///
/// | Field type | Port | Attributes |
/// | - | - | - |
/// | `f32` | any value in a range | `min` (0), `max` (1), `default` (`min`), `unit`, `logarithmic` |
/// | `i8` to `i32`, `u8` to `u32` | `lv2:integer` | `min`, `max` (both required, of the field's type, within ±2^24), `default` (`min`), `unit` |
/// | `bool` | `lv2:toggled` | `default` (`false`) |
/// | a [`Choice`] enum | `lv2:enumeration` | `default` (the first variant) |
///
/// `unit` names a [`Unit`] variant, as `unit = Hertz`; `logarithmic` needs [`Hertz`](Unit::Hertz),
/// [`Millisecond`](Unit::Millisecond) or [`Second`](Unit::Second) and a minimum above 0. Any other
/// type name, an alias included, is taken for a [`Choice`] enum.
///
/// | Option, each once | Means |
/// | - | - |
/// | `name` | the display name; the title-cased field name by default |
/// | `output` | the plugin writes the field |
/// | `quick_pot` | `dg:quickPot` on one input that is not `hidden` |
/// | `hidden` | `pprops:notOnGUI` |
/// | `expensive` | `pprops:expensive`, on an input |
/// | `saved` | `dg:savedToPreset`, on a `hidden` input |
/// | `blockable` | `dg:mayUpdateBlockedState` on an input, for [`Context::set_state`](crate::Context::set_state) |
///
/// The field name is the port symbol (`r#type` gives `type`): ASCII, and not `reset`,
/// `lv2_enabled`, `lv2_bpm`, `lv2_audio_in_<n>` or `lv2_audio_out_<n>`. The derive adds an
/// upper-case `const` [`Field`](crate::Field) per field and `Default`.
///
/// ```
/// use crabgram::prelude::*;
///
/// #[derive(Debug, PartialEq, Choice)]
/// pub enum Shape {
///     Sine,
///     Square,
/// }
///
/// #[derive(Controls)]
/// pub struct Params {
///     #[control(default = 3.0, min = 0.1, max = 12.0, unit = Hertz, logarithmic, quick_pot)]
///     pub rate: f32,
///     #[control(default = Shape::Square)]
///     pub shape: Shape,
///     #[control(min = 0, max = 16, unit = Semitone)]
///     pub steps: u32,
///     pub sync: bool,
///     #[control(output, name = "LFO")]
///     pub lfo: f32,
/// }
///
/// let params = Params::default();
/// assert_eq!(
///     (params.rate, params.shape, params.steps, params.sync),
///     (3.0, Shape::Square, 0, false)
/// );
///
/// assert_eq!(Params::CONTROLS.len(), 5);
/// assert_eq!(Params::LFO.as_ref().name(), "LFO");
/// assert_eq!(Params::STEPS.as_ref().value_count(), Some(17));
/// assert_eq!(Params::QUICK_POT, Some(Params::RATE.as_ref()));
///
/// let read = Params::from_ports(&[50.0, 0.4, 7.6, 1.0, 0.0]);
/// assert_eq!(
///     (read.rate, read.shape, read.steps, read.sync),
///     (12.0, Shape::Sine, 8, true)
/// );
///
/// assert_eq!(
///     Params { lfo: 0.25, ..read }.to_ports(),
///     [12.0, 0.0, 8.0, 1.0, 0.25]
/// );
/// ```
///
/// A float integer range fails to compile:
///
/// ```compile_fail,E0308
/// #[derive(crabgram::Controls)]
/// struct Params {
///     #[control(min = 0.0, max = 16.0)]
///     steps: u32,
/// }
/// ```
pub trait Controls: Default + crate::sealed::SealedControls + 'static {
    /// Every control, in port order.
    const CONTROLS: &'static [Control];

    /// The quick pot control, `dg:quickPot`: one of [`CONTROLS`](Self::CONTROLS).
    const QUICK_POT: Option<&'static Control>;

    /// One port value per control: `[f32; N]`.
    type Values: Copy + AsRef<[f32]> + AsMut<[f32]> + 'static;

    /// Reads the controls from the port values `raw`, each clamped to its range.
    fn from_ports(raw: &Self::Values) -> Self;

    /// The port value of every field.
    fn to_ports(&self) -> Self::Values;
}
