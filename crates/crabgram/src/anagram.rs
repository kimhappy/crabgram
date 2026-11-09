//! The sample rate, block size and tempo range.

/// The sample rate.
pub const SAMPLE_RATE: f64 = 48_000.0;

/// The block size in frames.
pub const FRAMES: usize = 16;

/// The slowest tempo, in beats per minute.
pub const MIN_BPM: f32 = 40.0;

/// The fastest tempo, in beats per minute.
pub const MAX_BPM: f32 = 300.0;
