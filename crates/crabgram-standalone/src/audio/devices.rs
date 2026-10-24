//! The audio devices to pick from, what each can do, and the settings picked.

use std::{
    collections::BTreeSet,
    fmt,
    ops::{Index, IndexMut},
};

use cpal::{
    BufferSize, Device, SampleFormat, SampleRate, StreamConfig, SupportedBufferSize,
    SupportedStreamConfigRange,
    traits::{DeviceTrait, HostTrait},
};

use super::{AudioError, SAMPLE_RATE};

pub const COMMON_RATES: [u32; 11] = [
    8_000, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000,
];

pub const COMMON_BUFFERS: [u32; 10] = [16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dir {
    Input,
    Output,
}

impl Dir {
    pub const BOTH: [Self; 2] = [Self::Input, Self::Output];

    /// Position in [`Self::BOTH`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for Dir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Input => "input",
            Self::Output => "output",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pick {
    Default,
    Device(String),
}

impl fmt::Display for Pick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Default => "System default",
            Self::Device(name) => name,
        })
    }
}

/// A channel of the plugin, from 0.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PluginChannel(pub usize);

/// A channel of a device, from 0.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct DeviceChannel(pub usize);

fn names(devices: impl Iterator<Item = Device>) -> Vec<String> {
    devices.filter_map(|device| device.name().ok()).collect()
}

/// The names of the devices of `dir`.
pub(super) fn device_names(dir: Dir) -> Vec<String> {
    let host = cpal::default_host();

    match dir {
        Dir::Input => host.input_devices().map(names),
        Dir::Output => host.output_devices().map(names),
    }
    .unwrap_or_default()
}

fn find(mut devices: impl Iterator<Item = Device>, name: &str) -> Option<Device> {
    devices.find(|device| device.name().is_ok_and(|it| it == name))
}

/// The device `pick` names in `dir`.
pub(super) fn device(dir: Dir, pick: &Pick) -> Result<Device, AudioError> {
    let host = cpal::default_host();

    match (pick, dir) {
        (Pick::Default, Dir::Input) => host.default_input_device(),
        (Pick::Default, Dir::Output) => host.default_output_device(),
        (Pick::Device(name), Dir::Input) => host.input_devices().ok().and_then(|it| find(it, name)),
        (Pick::Device(name), Dir::Output) => {
            host.output_devices().ok().and_then(|it| find(it, name))
        }
    }
    .ok_or_else(|| match pick {
        Pick::Device(name) => AudioError::NoDevice {
            dir,
            name: name.clone(),
        },
        Pick::Default => AudioError::NoDefaultDevice { dir },
    })
}

/// Converted sample formats, most precise first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Format {
    F32,
    I32,
    I16,
}

impl Format {
    pub(super) const fn of(format: SampleFormat) -> Option<Self> {
        match format {
            SampleFormat::F32 => Some(Self::F32),
            SampleFormat::I32 => Some(Self::I32),
            SampleFormat::I16 => Some(Self::I16),
            _ => None,
        }
    }
}

fn supports(range: &SupportedStreamConfigRange, rate: u32) -> bool {
    (range.min_sample_rate().0..=range.max_sample_rate().0).contains(&rate)
}

/// A stream configuration range in a converted [`Format`].
type Mode = (Format, SupportedStreamConfigRange);

/// The ranges of `device` in a converted [`Format`].
fn modes(device: &Device, dir: Dir) -> Vec<Mode> {
    match dir {
        Dir::Input => device
            .supported_input_configs()
            .map(Iterator::collect::<Vec<_>>),
        Dir::Output => device
            .supported_output_configs()
            .map(Iterator::collect::<Vec<_>>),
    }
    .unwrap_or_default()
    .into_iter()
    .filter_map(|range| Some((Format::of(range.sample_format())?, range)))
    .collect()
}

/// The common rates some mode supports, else the ends of every mode.
fn rates(modes: &[Mode]) -> Vec<u32> {
    let common: Vec<u32> = COMMON_RATES
        .into_iter()
        .filter(|&rate| modes.iter().any(|(_, it)| supports(it, rate)))
        .collect();

    if common.is_empty() {
        modes
            .iter()
            .flat_map(|(_, it)| [it.min_sample_rate().0, it.max_sample_rate().0])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    } else {
        common
    }
}

/// What a device can do: its modes, and the rates among them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Caps {
    modes: Vec<Mode>,
    rates: Vec<u32>,
}

impl Caps {
    /// Opens the device `pick` names to read what it can do; nothing for no device or one that
    /// cannot be read.
    #[must_use]
    pub fn probe(dir: Dir, pick: &Pick) -> Self {
        Self::from_modes(device(dir, pick).map_or_default(|device| modes(&device, dir)))
    }

    fn from_modes(modes: Vec<Mode>) -> Self {
        Self {
            rates: rates(&modes),
            modes,
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn of_ranges(ranges: &[SupportedStreamConfigRange]) -> Self {
        Self::from_modes(
            ranges
                .iter()
                .filter_map(|range| Some((Format::of(range.sample_format())?, *range)))
                .collect(),
        )
    }

    #[must_use]
    pub fn rates(&self) -> &[u32] {
        &self.rates
    }

    fn at(&self, rate: u32) -> impl Iterator<Item = &Mode> {
        self.modes.iter().filter(move |(_, it)| supports(it, rate))
    }

    /// The most channels a mode at `rate` has.
    #[must_use]
    pub fn channels(&self, rate: u32) -> usize {
        self.at(rate)
            .map(|(_, it)| usize::from(it.channels()))
            .max()
            .unwrap_or(0)
    }

    /// The mode a stream at `rate` with `needed` channels opens in: the fewest channels in the most
    /// precise format.
    fn mode(&self, rate: u32, needed: usize) -> Option<&Mode> {
        self.at(rate)
            .filter(|(_, it)| usize::from(it.channels()) >= needed)
            .min_by_key(|(format, it)| (*format, it.channels()))
    }

    /// The common buffer sizes the mode of `settings` takes, besides the device's own default.
    #[must_use]
    pub fn buffers(&self, settings: &Settings) -> Vec<u32> {
        self.mode(settings.rate, settings.needed())
            .map_or_default(|(_, it)| match *it.buffer_size() {
                SupportedBufferSize::Range { min, max } => COMMON_BUFFERS
                    .into_iter()
                    .filter(|it| (min..=max).contains(it))
                    .collect(),
                SupportedBufferSize::Unknown => Vec::new(),
            })
    }

    #[must_use]
    pub fn default_rate(&self) -> u32 {
        self.rates
            .iter()
            .copied()
            .min_by_key(|rate| rate.abs_diff(SAMPLE_RATE))
            .unwrap_or(SAMPLE_RATE)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    pick: Pick,
    rate: u32,
    /// Frames, or the device's default.
    buffer: Option<u32>,
    /// The device channel of each plugin channel.
    channels: Vec<DeviceChannel>,
}

impl Settings {
    /// Defaults for a device not read yet: the plugin's rate, the device's buffer size, its first
    /// channels. [`Self::fitted`] makes them what the device can do.
    #[must_use]
    pub fn unread(pick: Pick, plugin_channels: usize) -> Self {
        Self {
            pick,
            rate: SAMPLE_RATE,
            buffer: None,
            channels: (0..plugin_channels).map(DeviceChannel).collect(),
        }
    }

    /// These settings where `caps` supports them, else the nearest it does: the rate closest to
    /// the plugin's, the last channel, the device's buffer size.
    #[must_use]
    pub fn fitted(self, caps: &Caps) -> Self {
        let rate = if caps.rates.contains(&self.rate) {
            self.rate
        } else {
            caps.default_rate()
        };
        let last = DeviceChannel(caps.channels(rate).saturating_sub(1));
        let fitted = Self {
            rate,
            channels: self
                .channels
                .into_iter()
                .map(|channel| channel.min(last))
                .collect(),
            ..self
        };

        Self {
            buffer: fitted
                .buffer
                .filter(|buffer| caps.buffers(&fitted).contains(buffer)),
            ..fitted
        }
    }

    #[must_use]
    pub const fn pick(&self) -> &Pick {
        &self.pick
    }

    #[must_use]
    pub const fn rate(&self) -> u32 {
        self.rate
    }

    #[must_use]
    pub const fn buffer(&self) -> Option<u32> {
        self.buffer
    }

    #[must_use]
    pub fn channels(&self) -> &[DeviceChannel] {
        &self.channels
    }

    /// The device channel `plugin` goes to or comes from.
    #[must_use]
    pub fn channel(&self, plugin: PluginChannel) -> DeviceChannel {
        self.channels
            .get(plugin.0)
            .copied()
            .unwrap_or(DeviceChannel(0))
    }

    #[must_use]
    pub fn with_rate(self, rate: u32) -> Self {
        Self { rate, ..self }
    }

    #[must_use]
    pub fn with_buffer(self, buffer: Option<u32>) -> Self {
        Self { buffer, ..self }
    }

    #[must_use]
    pub fn with_channel(mut self, plugin: PluginChannel, device: DeviceChannel) -> Self {
        if let Some(channel) = self.channels.get_mut(plugin.0) {
            *channel = device;
        }
        self
    }

    pub(super) fn needed(&self) -> usize {
        self.channels.iter().max().map_or(1, |it| it.0 + 1)
    }
}

/// For the status line: rate and buffer.
impl fmt::Display for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.rate, buffer_label(self.buffer))
    }
}

/// Frames, or "auto" for the device's default.
#[must_use]
pub fn buffer_label(buffer: Option<u32>) -> String {
    buffer.map_or_else(|| "auto".to_owned(), |it| it.to_string())
}

/// One value for the input and one for the output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PerDir<T>([T; 2]);

impl<T> PerDir<T> {
    pub fn from_fn(value: impl FnMut(Dir) -> T) -> Self {
        Self(Dir::BOTH.map(value))
    }

    pub fn map<U>(self, value: impl FnMut(T) -> U) -> PerDir<U> {
        PerDir(self.0.map(value))
    }
}

impl<T> Index<Dir> for PerDir<T> {
    type Output = T;

    fn index(&self, dir: Dir) -> &T {
        &self.0[dir.index()]
    }
}

impl<T> IndexMut<Dir> for PerDir<T> {
    fn index_mut(&mut self, dir: Dir) -> &mut T {
        &mut self.0[dir.index()]
    }
}

/// The settings of the input, `None` for silence, and of the output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Io {
    pub input: Option<Settings>,
    pub output: Settings,
}

impl Io {
    /// The settings of `dir`; none for the silent input.
    #[must_use]
    pub const fn get(&self, dir: Dir) -> Option<&Settings> {
        match dir {
            Dir::Input => self.input.as_ref(),
            Dir::Output => Some(&self.output),
        }
    }

    pub const fn get_mut(&mut self, dir: Dir) -> Option<&mut Settings> {
        match dir {
            Dir::Input => self.input.as_mut(),
            Dir::Output => Some(&mut self.output),
        }
    }

    /// Each setting fitted to the `caps` of its direction.
    #[must_use]
    pub fn fitted(self, caps: &PerDir<Caps>) -> Self {
        Self {
            input: self.input.map(|it| it.fitted(&caps[Dir::Input])),
            output: self.output.fitted(&caps[Dir::Output]),
        }
    }
}

/// The fewest channels reaching the picked ones, in the most precise format having them.
pub(super) fn stream_config(
    caps: &Caps,
    dir: Dir,
    settings: &Settings,
) -> Result<(StreamConfig, Format), AudioError> {
    let (needed, rate) = (settings.needed(), settings.rate);

    caps.mode(rate, needed)
        .map(|&(format, range)| {
            let config = StreamConfig {
                buffer_size: settings
                    .buffer
                    .map_or(BufferSize::Default, BufferSize::Fixed),
                ..range.with_sample_rate(SampleRate(rate)).config()
            };

            (config, format)
        })
        .ok_or(AudioError::NoMode {
            dir,
            rate,
            channels: needed,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(channels: u16, rates: (u32, u32), buffers: (u32, u32)) -> SupportedStreamConfigRange {
        SupportedStreamConfigRange::new(
            channels,
            SampleRate(rates.0),
            SampleRate(rates.1),
            SupportedBufferSize::Range {
                min: buffers.0,
                max: buffers.1,
            },
            SampleFormat::F32,
        )
    }

    #[test]
    fn channels_and_buffers_are_those_of_the_mode_a_stream_opens() {
        let caps = Caps::of_ranges(&[
            range(2, (44_100, 48_000), (64, 1024)),
            range(8, (96_000, 96_000), (256, 256)),
        ]);
        let mut settings = Settings::unread(Pick::Default, 2).fitted(&caps);

        assert_eq!((settings.rate, caps.channels(settings.rate)), (48_000, 2));
        assert_eq!(caps.buffers(&settings), [64, 128, 256, 512, 1024]);

        settings.rate = 96_000;
        assert_eq!(
            (caps.channels(96_000), caps.buffers(&settings)),
            (8, vec![256])
        );
    }

    #[test]
    fn settings_describe_their_rate_and_buffer() {
        let caps = Caps::of_ranges(&[range(2, (48_000, 48_000), (64, 64))]);

        assert_eq!(
            Settings::unread(Pick::Default, 1).fitted(&caps).to_string(),
            "48000/auto"
        );
    }

    fn channels(channels: &[usize]) -> Vec<DeviceChannel> {
        channels.iter().copied().map(DeviceChannel).collect()
    }

    #[test]
    fn fitting_keeps_every_setting_the_device_supports() {
        let caps = Caps::of_ranges(&[
            range(2, (44_100, 48_000), (64, 1024)),
            range(4, (96_000, 96_000), (256, 256)),
        ]);
        let chosen = Settings {
            pick: Pick::Default,
            rate: 96_000,
            buffer: Some(256),
            channels: channels(&[3, 1]),
        };

        assert_eq!(chosen.clone().fitted(&caps), chosen);
        assert_eq!(
            Settings {
                rate: 44_100,
                ..chosen.clone()
            }
            .fitted(&caps),
            Settings {
                rate: 44_100,
                channels: channels(&[1, 1]),
                ..chosen.clone()
            }
        );
        assert_eq!(
            Settings {
                rate: 22_050,
                buffer: Some(2_048),
                ..chosen.clone()
            }
            .fitted(&caps),
            Settings {
                rate: 48_000,
                buffer: None,
                channels: channels(&[1, 1]),
                ..chosen
            }
        );
    }

    #[test]
    fn unread_settings_fit_to_the_defaults_of_the_device() {
        let stereo = Caps::of_ranges(&[range(2, (44_100, 96_000), (64, 64))]);
        let mono = Caps::of_ranges(&[range(1, (44_100, 44_100), (64, 64))]);
        let unread = Settings::unread(Pick::Default, 2);

        assert_eq!(
            (
                unread.clone().fitted(&stereo).rate,
                unread.clone().fitted(&stereo).channels
            ),
            (48_000, channels(&[0, 1]))
        );
        assert_eq!(
            (
                unread.clone().fitted(&mono).rate,
                unread.fitted(&mono).channels
            ),
            (44_100, channels(&[0, 0]))
        );
    }

    #[test]
    fn the_most_precise_convertible_format_ranks_first() {
        let formats = [
            SampleFormat::U8,
            SampleFormat::I16,
            SampleFormat::F64,
            SampleFormat::I32,
            SampleFormat::F32,
        ];

        assert_eq!(
            formats.into_iter().filter_map(Format::of).min(),
            Some(Format::F32)
        );
        assert_eq!(
            formats.into_iter().filter_map(Format::of).max(),
            Some(Format::I16)
        );
        assert_eq!(Format::of(SampleFormat::U8), None);
    }
}
