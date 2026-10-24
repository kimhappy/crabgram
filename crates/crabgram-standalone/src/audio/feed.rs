//! What the output callback owns: the input ring as the plugin reads it, and the plugin run
//! between the resamplers.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use cpal::{FromSample, SizedSample};
use crabgram::anagram::FRAMES;
use ringbuf::traits::{Consumer, Observer};

use super::{DeviceChannel, SAMPLE_RATE, denormals::FlushDenormals, resample::Resampler};
use crate::plugin::Lent;

/// Ring capacity per channel.
pub const RING_FRAMES: usize = 65_536;

/// Share of the gap to the measured fill that the smoothed fill closes per block.
const SMOOTHING: f64 = 1.0 / 6_000.0;

const DRIFT_GAIN: f64 = 0.002;

/// How fast a lasting excess builds up into drift.
const DRIFT_BUILDUP: f64 = DRIFT_GAIN / 30_000.0;

const MAX_DRIFT: f64 = 0.005;

/// A ring this many times over its target drops back to it; drift is too slow to work off a
/// backlog.
const OVERFILL: f64 = 2.5;

/// Seconds over which a past longest call fades.
const BURST_FADE_SECONDS: f64 = 10.0;

/// The input ring as the plugin reads it: silent until it holds its target.
pub struct Feed<C> {
    ring: C,
    channels: usize,
    rate: u32,
    playing: bool,
    /// Smoothed over blocks.
    level: f64,
    lasting: f64,
    /// The longest input call since the output last looked, from the input callback.
    input_calls: Arc<AtomicUsize>,
    /// The longest input call, fading.
    input_burst: f64,
    /// The longest output call at the input's rate, fading.
    output_burst: f64,
    /// Input frames over which a past longest call fades, [`BURST_FADE_SECONDS`] long.
    burst_fade: f64,
}

impl<C: Consumer<Item = f32> + Observer> Feed<C> {
    /// Silent until `ring` of `channels` interleaved channels at `input_rate` fills.
    pub fn new(ring: C, channels: usize, input_rate: u32, input_calls: Arc<AtomicUsize>) -> Self {
        Self {
            ring,
            channels,
            rate: input_rate,
            playing: false,
            level: 0.0,
            lasting: 0.0,
            input_burst: input_calls.load(Ordering::Relaxed) as f64,
            input_calls,
            output_burst: 0.0,
            burst_fade: BURST_FADE_SECONDS * f64::from(input_rate),
        }
    }

    /// Takes the longest input call since the last output call and `output_call`, the input frames
    /// this output call takes; older longest calls fade over [`BURST_FADE_SECONDS`].
    fn measure(&mut self, output_call: usize) {
        let fade = 1.0 - output_call as f64 / self.burst_fade;
        let input_call = self.input_calls.swap(0, Ordering::Relaxed);

        self.input_burst = (self.input_burst * fade).max(input_call as f64);
        self.output_burst = (self.output_burst * fade).max(output_call as f64);
    }

    fn available(&self) -> usize {
        self.ring.occupied_len() / self.channels
    }

    /// The fill to hold: room for the longest calls of both streams, two blocks at least.
    fn target(&self) -> f64 {
        let blocks = (2 * FRAMES) as f64;
        let wanted = 1.5 * (self.input_burst + self.output_burst) + blocks;
        let room = (self.ring.capacity().get() / self.channels) as f64 - self.input_burst;

        wanted.min(room.max(blocks)).round()
    }

    /// Readies the ring for the next block; returns the read-rate factor that holds the target.
    fn prepare(&mut self) -> f64 {
        let target = self.target();
        let available = self.available();
        let drift = self.drift(available, target);
        let resync_at = if self.playing {
            OVERFILL * target
        } else {
            target
        };

        if available as f64 >= resync_at {
            self.resync(available, target);
        }

        drift
    }

    /// The next frame in the ring, or silence while it is not playing.
    fn fetch(&mut self, frame: &mut [f32]) {
        let popped = if self.playing {
            self.ring.pop_slice(frame)
        } else {
            0
        };

        self.playing = popped > 0;
        frame[popped..].fill(0.0);
    }

    /// Plays on from the newest `target` of the `available` frames, dropping the rest; keeps the
    /// learned drift.
    fn resync(&mut self, available: usize, target: f64) {
        self.ring
            .skip((available - target as usize) * self.channels);
        self.playing = true;
        self.level = target;
    }

    /// Read-rate factor that holds `target` with `available` frames in the ring.
    fn drift(&mut self, available: usize, target: f64) -> f64 {
        if !self.playing {
            return 1.0;
        }

        self.level += (available as f64 - self.level) * SMOOTHING;

        let excess = (self.level - target) / target;

        self.lasting = (self.lasting + DRIFT_BUILDUP * excess).clamp(-MAX_DRIFT, MAX_DRIFT);
        1.0 + (DRIFT_GAIN * excess + self.lasting).clamp(-MAX_DRIFT, MAX_DRIFT)
    }
}

struct Source<C> {
    instance: Lent,
    /// The input, and the resampler that brings it to the plugin's rate.
    feed: Option<(Feed<C>, Resampler)>,
    /// A block of frames, channels interleaved, on its way into or out of the plugin.
    block: Vec<f32>,
    /// The next output frame of the block last run.
    cursor: usize,
}

impl<C: Consumer<Item = f32> + Observer> Source<C> {
    /// The next output frame, running another block when the last is used up.
    fn next_output(&mut self, frame: &mut [f32]) {
        if self.cursor == FRAMES {
            self.run_block();
            self.cursor = 0;
        }

        let channels = frame.len();

        frame.copy_from_slice(&self.block[self.cursor * channels..][..channels]);
        self.cursor += 1;
    }

    fn run_block(&mut self) {
        let channels = self.instance.channels();

        if let Some((feed, from_input)) = &mut self.feed {
            from_input.set_drift(feed.prepare());

            let mut fetch = |buffer: &mut [f32]| feed.fetch(buffer);

            self.block
                .chunks_exact_mut(channels)
                .for_each(|frame| from_input.produce(&mut fetch, frame));

            for channel in 0..channels {
                let port = self.instance.input(channel);

                for (cell, frame) in port.iter().zip(self.block.chunks_exact(channels)) {
                    cell.set(frame[channel]);
                }
            }
        }

        self.instance.run();

        for channel in 0..channels {
            let port = self.instance.output(channel);

            for (cell, frame) in port.iter().zip(self.block.chunks_exact_mut(channels)) {
                frame[channel] = cell.get();
            }
        }
    }
}

/// Everything the output callback owns.
pub struct Dsp<C> {
    source: Source<C>,
    to_output: Resampler,
    input_per_output: f64,
    frame: Vec<f32>,
    /// Device channel of each plugin channel, and its share.
    output_map: Vec<(DeviceChannel, f32)>,
    /// Mixed in `f32` before conversion.
    mixed: Vec<f32>,
}

impl<C: Consumer<Item = f32> + Observer> Dsp<C> {
    /// Runs `instance` on `feed`, or on silence without one, into the `output_map` device channel
    /// of each plugin channel at `output_rate`.
    pub fn new(
        instance: Lent,
        feed: Option<Feed<C>>,
        output_rate: u32,
        output_map: &[DeviceChannel],
        output_channels: usize,
    ) -> Self {
        let channels = instance.channels();

        if feed.is_none() {
            (0..channels)
                .flat_map(|channel| instance.input(channel))
                .for_each(|cell| cell.set(0.0));
        }

        Self {
            input_per_output: feed
                .as_ref()
                .map_or(0.0, |feed| f64::from(feed.rate) / f64::from(output_rate)),
            source: Source {
                instance,
                feed: feed.map(|feed| {
                    let from_input = Resampler::drifting(channels, feed.rate, SAMPLE_RATE);

                    (feed, from_input)
                }),
                block: vec![0.0; channels * FRAMES],
                cursor: FRAMES,
            },
            to_output: Resampler::fixed(channels, SAMPLE_RATE, output_rate),
            frame: vec![0.0; channels],
            output_map: shares(output_map),
            mixed: vec![0.0; output_channels],
        }
    }

    /// Fills `data` with denormals flushed to zero, restoring the thread's float mode after.
    pub fn fill<T: SizedSample + FromSample<f32>>(&mut self, data: &mut [T]) {
        let _flushing = FlushDenormals::begin();
        let Self {
            source,
            to_output,
            input_per_output,
            frame,
            output_map,
            mixed,
        } = self;

        if let Some((feed, _)) = &mut source.feed {
            let frames = data.len() / mixed.len();
            let taken = (frames as f64 * *input_per_output).ceil() as usize;

            feed.measure(taken);
        }

        for device_frame in data.chunks_exact_mut(mixed.len()) {
            to_output.produce(&mut |buffer: &mut [f32]| source.next_output(buffer), frame);
            mix(frame, output_map, mixed, device_frame);
        }
    }
}

/// Mixes the plugin's `frame` into `device_frame` by `output_map`, through `mixed`; a sample that
/// is not finite becomes silence, the rest stays within full scale.
fn mix<T: SizedSample + FromSample<f32>>(
    frame: &[f32],
    output_map: &[(DeviceChannel, f32)],
    mixed: &mut [f32],
    device_frame: &mut [T],
) {
    mixed.fill(0.0);

    for (&(device, share), sample) in output_map.iter().zip(frame) {
        if let Some(channel) = mixed.get_mut(device.0) {
            *channel += sample * share;
        }
    }

    for (sample, mixed) in device_frame.iter_mut().zip(mixed.iter()) {
        let safe = if mixed.is_finite() {
            mixed.clamp(-1.0, 1.0)
        } else {
            0.0
        };

        *sample = T::from_sample(safe);
    }
}

/// Pairs each plugin channel's device channel with its share; plugin channels meeting on one
/// device channel are averaged.
fn shares(channels: &[DeviceChannel]) -> Vec<(DeviceChannel, f32)> {
    channels
        .iter()
        .map(|&device| {
            let meeting = channels.iter().filter(|&&it| it == device).count();

            (device, 1.0 / meeting as f32)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ringbuf::{
        HeapRb,
        traits::{Producer, Split},
    };

    use super::*;
    use crate::{audio::SAMPLE_RATE, plugin::Shelf};

    /// Returns how often the ring ran dry after first playing, and its final fill in targets.
    fn follow(ppm: f64, burst: usize, backlog: usize, seconds: f64) -> (usize, f64) {
        let (mut producer, consumer) = HeapRb::<f32>::new(RING_FRAMES).split();
        (0..backlog).for_each(|_| {
            let _ = producer.try_push(0.0);
        });
        let mut feed = Feed {
            output_burst: 256.0,
            ..Feed::new(consumer, 1, SAMPLE_RATE, Arc::new(AtomicUsize::new(burst)))
        };
        let mut resampler = Resampler::drifting(1, SAMPLE_RATE, SAMPLE_RATE);
        let input_rate = f64::from(SAMPLE_RATE) * (1.0 + ppm * 1.0e-6);
        let blocks = (seconds * f64::from(SAMPLE_RATE) / FRAMES as f64) as usize;
        let (mut pushed, mut dry, mut played) = (0, 0, false);

        for block in 1..=blocks {
            let due = ((block * FRAMES) as f64 * input_rate / f64::from(SAMPLE_RATE)) as usize;

            while pushed + burst <= due {
                pushed += burst;
                (0..burst).for_each(|_| {
                    let _ = producer.try_push(0.0);
                });
            }

            resampler.set_drift(feed.prepare());

            for _ in 0..FRAMES {
                resampler.produce(&mut |frame: &mut [f32]| feed.fetch(frame), &mut [0.0]);
            }

            played |= feed.playing;
            dry += usize::from(played && !feed.playing);
        }

        (dry, feed.available() as f64 / feed.target())
    }

    fn shares_of(channels: &[usize]) -> Vec<(usize, f32)> {
        let channels: Vec<_> = channels.iter().copied().map(DeviceChannel).collect();

        shares(&channels)
            .into_iter()
            .map(|(device, share)| (device.0, share))
            .collect()
    }

    #[test]
    fn channels_that_meet_on_one_device_channel_are_averaged() {
        assert_eq!(shares_of(&[0, 1]), [(0, 1.0), (1, 1.0)]);
        assert_eq!(shares_of(&[0, 0]), [(0, 0.5), (0, 0.5)]);
        assert_eq!(shares_of(&[1, 0, 1]), [(1, 0.5), (0, 1.0), (1, 0.5)]);
    }

    #[test]
    fn an_instance_run_without_a_feed_hears_silence_rather_than_its_last_input() {
        let host = crate::plugin::tests::passing_host();
        let shelf = Shelf::default();
        let earlier = shelf.lend(&host).unwrap();

        earlier.input(0).iter().for_each(|cell| cell.set(0.5));
        drop(earlier);

        let mut dsp = Dsp::new(
            shelf.lend(&host).unwrap(),
            None::<Feed<<HeapRb<f32> as Split>::Cons>>,
            SAMPLE_RATE,
            &[DeviceChannel(0)],
            1,
        );
        let mut output = [1.0_f32; 4 * FRAMES];

        dsp.fill(&mut output);

        assert!(output.iter().all(|&sample| sample == 0.0));
    }

    #[test]
    fn the_device_gets_silence_for_a_broken_sample_and_full_scale_at_most() {
        let stub = [f32::NAN, 10.0, -10.0, 0.5];
        let mut mixed = [0.0; 4];
        let mut device = [1.0_f32; 4];
        let mut integer = [i16::MAX; 4];

        mix(
            &stub,
            &shares(&[0, 1, 2, 3].map(DeviceChannel)),
            &mut mixed,
            &mut device,
        );
        mix(
            &stub,
            &shares(&[0, 1, 2, 3].map(DeviceChannel)),
            &mut mixed,
            &mut integer,
        );

        assert_eq!(device, [0.0, 1.0, -1.0, 0.5]);
        assert_eq!(integer[0], 0);
        assert_eq!(integer[1], i16::MAX);
    }

    #[test]
    fn a_backlog_at_the_start_never_runs_the_ring_dry() {
        let (dry, fill) = follow(0.0, 128, 4_800, 60.0);

        assert_eq!(dry, 0);
        assert!((0.8..1.25).contains(&fill), "fill {fill}");
    }

    #[test]
    fn a_fast_input_clock_keeps_the_ring_near_its_target() {
        let (dry, fill) = follow(1_000.0, 128, 0, 60.0);

        assert_eq!(dry, 0);
        assert!((0.8..1.25).contains(&fill), "fill {fill}");
    }

    #[test]
    fn a_playing_ring_at_its_overfill_drops_back_to_its_newest_frames() {
        let (mut producer, consumer) = HeapRb::<f32>::new(RING_FRAMES).split();
        let mut feed = Feed {
            output_burst: 256.0,
            ..Feed::new(consumer, 1, SAMPLE_RATE, Arc::new(AtomicUsize::new(128)))
        };
        let target = feed.target() as usize;
        let overfill = (OVERFILL * target as f64).ceil() as usize;
        let mut pushed = 0;
        let mut fill_to = |frames: usize| {
            while producer.occupied_len() < frames {
                let _ = producer.try_push(pushed as f32);
                pushed += 1;
            }
        };
        let mut frame = [0.0];

        let mut next_block = |feed: &mut Feed<_>| {
            feed.prepare();
            feed.fetch(&mut frame);
            frame
        };

        fill_to(target);
        next_block(&mut feed);
        fill_to(overfill - 1);

        assert_eq!(next_block(&mut feed), [1.0]);

        fill_to(overfill);
        let frame = next_block(&mut feed);

        assert_eq!(frame, [(pushed - target) as f32]);
        assert_eq!(feed.available(), target - 1);
    }

    #[test]
    fn a_ring_too_small_for_the_bursts_still_plays() {
        let (mut producer, consumer) = HeapRb::<f32>::new(1_024).split();
        let mut feed = Feed {
            output_burst: 4_096.0,
            ..Feed::new(consumer, 1, SAMPLE_RATE, Arc::new(AtomicUsize::new(512)))
        };

        while producer.try_push(0.5).is_ok() {}
        feed.prepare();
        feed.fetch(&mut [0.0]);

        assert!(feed.playing);
    }

    #[test]
    fn a_lone_long_input_call_fades_from_the_target() {
        let calls = Arc::new(AtomicUsize::new(4_096));
        let (_, consumer) = HeapRb::<f32>::new(RING_FRAMES).split();
        let mut feed = Feed::new(consumer, 1, SAMPLE_RATE, Arc::clone(&calls));
        let raised = feed.target();
        let output_calls = 60 * SAMPLE_RATE as usize / 256;

        (0..output_calls).for_each(|_| {
            calls.fetch_max(128, Ordering::Relaxed);
            feed.measure(256);
        });

        assert_eq!(feed.input_burst, 128.0);
        assert_eq!(feed.output_burst, 256.0);
        assert!(
            feed.target() < raised / 4.0,
            "{} of {raised}",
            feed.target()
        );
    }

    #[test]
    fn a_slow_input_clock_never_runs_the_ring_dry() {
        let (dry, fill) = follow(-1_000.0, 128, 0, 60.0);

        assert_eq!(dry, 0);
        assert!((0.8..1.25).contains(&fill), "fill {fill}");
    }
}
