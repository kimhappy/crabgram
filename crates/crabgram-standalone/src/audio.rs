//! Audio devices, and the plugin run between an input and an output.
//!
//! Resamplers and ring buffers convert between the plugin's rate and each device's.

mod denormals;
mod devices;
mod feed;
mod resample;
mod worker;

use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};

use cpal::{
    Device, FromSample, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, StreamTrait},
};
#[cfg(test)]
pub use devices::{COMMON_BUFFERS, COMMON_RATES};
pub use devices::{
    Caps, DeviceChannel, Dir, Io, PerDir, Pick, PluginChannel, Settings, buffer_label,
};
use devices::{Format, device, device_names, stream_config};
use feed::{Dsp, Feed, RING_FRAMES};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
pub use worker::{Audio, Reply};

use crate::plugin::{Host, Shelf, StartError};

/// The plugin's sample rate.
pub const SAMPLE_RATE: u32 = crabgram::anagram::SAMPLE_RATE as u32;

/// Why the audio did not start or stopped.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no {dir} device {name}")]
    NoDevice { dir: Dir, name: String },
    #[error("no default {dir} device")]
    NoDefaultDevice { dir: Dir },
    #[error("the {dir} has no {rate} Hz mode with {channels} channels")]
    NoMode {
        dir: Dir,
        rate: u32,
        channels: usize,
    },
    #[error("cannot open the {dir} stream")]
    Open {
        dir: Dir,
        #[source]
        source: cpal::BuildStreamError,
    },
    #[error("cannot start the {dir} stream")]
    Play {
        dir: Dir,
        #[source]
        source: cpal::PlayStreamError,
    },
    #[error(transparent)]
    Plugin(#[from] StartError),
    #[error("{dir}")]
    Stream {
        dir: Dir,
        #[source]
        source: cpal::StreamError,
    },
    #[error("the audio thread stopped")]
    ThreadStopped,
}

/// The first error either stream reported while running.
type Fault = Arc<OnceLock<AudioError>>;

/// What the streams tell back: how many frames the input delivered at most per call, and the
/// first error either stream hit.
#[derive(Clone, Default)]
struct Reports {
    burst: Arc<AtomicUsize>,
    fault: Fault,
}

/// Runs `$stream` once for the sample type of `$format`, named `$sample` in it.
macro_rules! per_format {
    ($format:expr, $sample:ident => $stream:expr) => {
        match $format {
            Format::F32 => {
                type $sample = f32;
                $stream
            }
            Format::I32 => {
                type $sample = i32;
                $stream
            }
            Format::I16 => {
                type $sample = i16;
                $stream
            }
        }
    };
}

/// Pushes the `channels` of every frame into `producer` while it has room; records the longest
/// call in the burst of `reports`.
fn input_stream<T>(
    device: &Device,
    config: &StreamConfig,
    channels: Vec<DeviceChannel>,
    mut producer: impl Producer<Item = f32> + Send + 'static,
    Reports { burst, fault }: Reports,
) -> Result<Stream, AudioError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let device_channels = usize::from(config.channels);

    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let frames = data.chunks_exact(device_channels);
                let room = producer
                    .vacant_len()
                    .checked_div(channels.len())
                    .unwrap_or(0);

                burst.fetch_max(frames.len(), Ordering::Relaxed);
                producer.push_iter(frames.take(room).flat_map(|frame| {
                    channels
                        .iter()
                        .map(|channel| frame[channel.0].to_sample::<f32>())
                }));
            },
            move |source| {
                fault
                    .set(AudioError::Stream {
                        dir: Dir::Input,
                        source,
                    })
                    .ok();
            },
            None,
        )
        .map_err(|source| AudioError::Open {
            dir: Dir::Input,
            source,
        })
}

fn output_stream<T, C>(
    device: &Device,
    config: &StreamConfig,
    mut dsp: Dsp<C>,
    fault: Fault,
) -> Result<Stream, AudioError>
where
    T: SizedSample + FromSample<f32>,
    C: Consumer<Item = f32> + Observer + Send + 'static,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| dsp.fill(data),
            move |source| {
                fault
                    .set(AudioError::Stream {
                        dir: Dir::Output,
                        source,
                    })
                    .ok();
            },
            None,
        )
        .map_err(|source| AudioError::Open {
            dir: Dir::Output,
            source,
        })
}

/// The engine running on the audio thread, as the window sees it.
pub struct EngineStatus {
    input_error: Option<AudioError>,
    fault: Fault,
}

impl EngineStatus {
    /// Why the plugin runs on silence, if the input failed.
    #[must_use]
    pub const fn input_error(&self) -> Option<&AudioError> {
        self.input_error.as_ref()
    }

    /// The first error a stream reported while running.
    #[must_use]
    pub fn fault(&self) -> Option<&AudioError> {
        self.fault.get()
    }
}

struct Engine {
    _input: Option<Stream>,
    _output: Stream,
}

impl Engine {
    fn open_input(
        device: &Device,
        caps: &Caps,
        settings: &Settings,
        producer: impl Producer<Item = f32> + Send + 'static,
        reports: &Reports,
    ) -> Result<Stream, AudioError> {
        let (config, format) = stream_config(caps, Dir::Input, settings)?;
        let (channels, reports) = (settings.channels().to_vec(), reports.clone());
        let stream = per_format!(
            format,
            Sample => input_stream::<Sample>(device, &config, channels, producer, reports)
        )?;

        stream.play().map_err(|source| AudioError::Play {
            dir: Dir::Input,
            source,
        })?;
        Ok(stream)
    }

    /// Starts the plugin of `host`, the instance on `shelf` if one waits there, on the devices of
    /// `io`, which can do `caps`. A missing input runs the plugin on silence;
    /// [`EngineStatus::input_error`] says why.
    fn start(
        host: &Host,
        shelf: &Shelf,
        io: &Io,
        caps: &PerDir<Caps>,
    ) -> Result<(Self, EngineStatus), AudioError> {
        let output = &io.output;
        let output_device = device(Dir::Output, output.pick())?;
        let (output_config, output_format) =
            stream_config(&caps[Dir::Output], Dir::Output, output)?;
        let instance = shelf.lend(host)?;
        let channels = instance.channels();
        let (producer, consumer) = HeapRb::<f32>::new(RING_FRAMES * channels).split();
        let reports = Reports::default();
        let opened = io.input.as_ref().map(|input| {
            let device = match input.pick() {
                pick @ Pick::Device(_) if pick == output.pick() => Ok(output_device.clone()),
                pick => device(Dir::Input, pick),
            };

            device
                .and_then(|device| {
                    Self::open_input(&device, &caps[Dir::Input], input, producer, &reports)
                })
                .map(|stream| (stream, input.rate()))
        });
        let (input_stream, input_error) = match opened.transpose() {
            Ok(stream) => (stream, None),
            Err(error) => (None, Some(error)),
        };
        let dsp = Dsp::new(
            instance,
            input_stream
                .as_ref()
                .map(|&(_, rate)| Feed::new(consumer, channels, rate, reports.burst.clone())),
            output.rate(),
            output.channels(),
            usize::from(output_config.channels),
        );
        let output_stream = per_format!(
            output_format,
            Sample => output_stream::<Sample, _>(&output_device, &output_config, dsp, reports.fault.clone())
        )?;

        output_stream.play().map_err(|source| AudioError::Play {
            dir: Dir::Output,
            source,
        })?;

        Ok((
            Self {
                _input: input_stream.map(|(stream, _)| stream),
                _output: output_stream,
            },
            EngineStatus {
                input_error,
                fault: reports.fault,
            },
        ))
    }
}
