//! Port order: audio inputs, audio outputs, enabled (`lv2:enabled`), reset (`kx:Reset`), controls
//! in field order, tempo.

use core::ops::Range;
use std::borrow::Cow;

use crate::{Control, Controls, Plugin};

pub(crate) const ENABLED: &str = "lv2_enabled";
pub(crate) const RESET: &str = "reset";
pub(crate) const TEMPO: &str = "lv2_bpm";
/// The symbol of an audio input before its channel number, counted from 1.
pub(crate) const AUDIO_IN: &str = "lv2_audio_in_";
/// The symbol of an audio output before its channel number, counted from 1.
pub(crate) const AUDIO_OUT: &str = "lv2_audio_out_";

/// What a port is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    /// Audio input of channel `n`.
    AudioIn(u32),
    /// Audio output of channel `n`.
    AudioOut(u32),
    /// The enabled switch: 1 while on.
    Enabled,
    /// Reset trigger (`kx:Reset`).
    Reset,
    /// A control port.
    Control(Control),
    /// Input the host fills with its tempo (`time:beatsPerMinute`).
    Tempo,
}

impl Role {
    /// The port symbol.
    #[must_use]
    pub fn symbol(self) -> Cow<'static, str> {
        match self {
            Self::AudioIn(channel) => Cow::Owned(format!("{AUDIO_IN}{}", channel + 1)),
            Self::AudioOut(channel) => Cow::Owned(format!("{AUDIO_OUT}{}", channel + 1)),
            Self::Enabled => Cow::Borrowed(ENABLED),
            Self::Reset => Cow::Borrowed(RESET),
            Self::Control(control) => Cow::Borrowed(control.symbol()),
            Self::Tempo => Cow::Borrowed(TEMPO),
        }
    }

    /// The display name.
    #[must_use]
    pub fn name(self) -> Cow<'static, str> {
        match self {
            Self::AudioIn(channel) => Cow::Owned(format!("Audio Input {}", channel + 1)),
            Self::AudioOut(channel) => Cow::Owned(format!("Audio Output {}", channel + 1)),
            Self::Enabled => Cow::Borrowed("Enabled"),
            Self::Reset => Cow::Borrowed("Reset"),
            Self::Control(control) => Cow::Borrowed(control.name()),
            Self::Tempo => Cow::Borrowed("BPM"),
        }
    }
}

/// Channel and control counts, which fix every port index.
///
/// ```
/// use crabgram::port::Layout;
///
/// let layout = Layout::new(2, 3);
///
/// assert_eq!(
///     (layout.enabled(), layout.control(0), layout.tempo()),
///     (4, 6, 9)
/// );
/// assert_eq!(layout.position(7), Some(1));
/// assert_eq!(layout.position(9), None);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Layout {
    channels: u32,
    controls: u32,
}

impl Layout {
    /// A layout of `channels` and `controls`.
    #[must_use]
    pub const fn new(channels: u32, controls: u32) -> Self {
        Self { channels, controls }
    }

    /// Audio channels.
    #[must_use]
    pub const fn channels(self) -> u32 {
        self.channels
    }

    #[cfg(feature = "testing")]
    pub(crate) const fn controls(self) -> u32 {
        self.controls
    }

    /// The layout of `P`.
    #[must_use]
    pub const fn of<P: Plugin>() -> Self {
        Self::new(P::INFO.channels(), P::Controls::CONTROLS.len() as u32)
    }

    /// Index of the audio input of `channel`.
    #[must_use]
    pub const fn audio_in(self, channel: u32) -> u32 {
        channel
    }

    /// Index of the audio output of `channel`.
    #[must_use]
    pub const fn audio_out(self, channel: u32) -> u32 {
        self.channels + channel
    }

    /// Index of the enabled port.
    #[must_use]
    pub const fn enabled(self) -> u32 {
        2 * self.channels
    }

    /// Index of the reset trigger.
    #[must_use]
    pub const fn reset(self) -> u32 {
        self.enabled() + 1
    }

    /// Index of the control at `position`.
    #[must_use]
    pub const fn control(self, position: u32) -> u32 {
        self.reset() + 1 + position
    }

    /// The control position of port `index`, or `None` if it is no control.
    #[must_use]
    pub const fn position(self, index: u32) -> Option<u32> {
        match index.checked_sub(self.control(0)) {
            Some(position) if position < self.controls => Some(position),
            _ => None,
        }
    }

    pub(crate) const fn audio_inputs(self) -> Range<u32> {
        self.audio_in(0)..self.audio_out(0)
    }

    pub(crate) const fn audio_outputs(self) -> Range<u32> {
        self.audio_out(0)..self.enabled()
    }

    pub(crate) const fn control_ports(self) -> Range<u32> {
        self.control(0)..self.tempo()
    }

    /// Index of the tempo port.
    #[must_use]
    pub const fn tempo(self) -> u32 {
        self.control(self.controls)
    }

    /// The number of ports.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.tempo() + 1
    }
}

/// One port as the Turtle description lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    index: u32,
    role: Role,
}

impl Port {
    /// The port index.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    /// What the port is for.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }
}

/// Every port of `P`, by index.
///
/// ```
/// use crabgram::{
///     port::{self, Role},
///     prelude::*,
/// };
///
/// #[derive(Controls)]
/// struct Params {
///     gain: f32,
/// }
///
/// struct Gain;
///
/// impl Plugin for Gain {
///     type Controls = Params;
///
///     # const INFO: Info = info! {
///     #     uri: c"urn:example:gain",
///     #     name: "Gain",
///     #     description: "",
///     #     abbreviation: "GN",
///     #     category: Utility,
///     #     channels: 1,
///     # };
///     # fn new() -> Self { Self }
///     # fn run(&mut self, _: &Params, _: &mut Context<'_, Self>) {}
/// }
///
/// let ports = port::ports::<Gain>();
/// let symbols: Vec<_> = ports.iter().map(|port| port.role().symbol()).collect();
///
/// assert_eq!(
///     symbols,
///     [
///         "lv2_audio_in_1",
///         "lv2_audio_out_1",
///         "lv2_enabled",
///         "reset",
///         "gain",
///         "lv2_bpm"
///     ]
/// );
/// assert_eq!(ports[4].role(), Role::Control(*Params::GAIN.as_ref()));
/// assert!(ports.iter().map(port::Port::index).eq(0..6));
/// ```
#[must_use]
pub fn ports<P: Plugin>() -> Vec<Port> {
    let layout = Layout::of::<P>();
    let channels = 0..layout.channels;

    channels
        .clone()
        .map(|channel| (layout.audio_in(channel), Role::AudioIn(channel)))
        .chain(channels.map(|channel| (layout.audio_out(channel), Role::AudioOut(channel))))
        .chain([
            (layout.enabled(), Role::Enabled),
            (layout.reset(), Role::Reset),
        ])
        .chain(
            P::Controls::CONTROLS
                .iter()
                .zip(0..)
                .map(|(control, position)| (layout.control(position), Role::Control(*control))),
        )
        .chain([(layout.tempo(), Role::Tempo)])
        .map(|(index, role)| Port { index, role })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Layout;

    #[test]
    fn the_layout_places_every_port() {
        let layout = Layout::new(2, 3);

        assert_eq!((layout.audio_in(1), layout.audio_out(1)), (1, 3));
        assert_eq!((layout.reset(), layout.count()), (5, 10));
    }
}
