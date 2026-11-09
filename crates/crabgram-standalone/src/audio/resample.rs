//! Windowed-sinc sample rate conversion with a drifting ratio, low-passed to the lower rate.

use std::{f64::consts::PI, iter};

/// Frames of the lower rate on each side of an output frame.
const HALF: usize = 32;

/// Kernel table positions between two input frames.
const PHASES: usize = 256;

/// Cutoff as a share of the lower rate's Nyquist frequency.
const PASSBAND: f64 = 0.9;

/// Kaiser window shape.
const BETA: f64 = 8.0;

fn bessel_i0(x: f64) -> f64 {
    let quarter = x * x / 4.0;

    iter::successors(Some((1.0, 1.0_f64)), |&(term, k)| {
        Some((term * quarter / k.powi(2), k + 1.0))
    })
    .map(|(term, _)| term)
    .take_while(|term| *term > 1.0e-12)
    .sum()
}

/// Low-pass kernel at `x` input frames from the output frame, `half` input frames wide on each
/// side, up to a constant that [`table`] normalizes; `cutoff` is in cycles per input frame.
fn kernel(x: f64, half: usize, cutoff: f64) -> f64 {
    let window = 1.0 - (x / half as f64).powi(2);

    if window <= 0.0 {
        return 0.0;
    }

    let sinc = if x == 0.0 {
        1.0
    } else {
        (2.0 * PI * cutoff * x).sin() / (2.0 * PI * cutoff * x)
    };

    sinc * bessel_i0(BETA * window.sqrt())
}

/// Weights per position, `PHASES + 1` rows of `2 × half` taps one after the other, each row
/// summing to 1 so DC passes.
fn table(half: usize, cutoff: f64) -> Vec<f32> {
    (0..=PHASES)
        .flat_map(|phase| {
            let t = phase as f64 / PHASES as f64;
            let weights: Vec<f64> = (0..2 * half)
                .map(|tap| kernel(tap as f64 - (half - 1) as f64 - t, half, cutoff))
                .collect();
            let sum: f64 = weights.iter().sum();

            weights.into_iter().map(move |it| (it / sum) as f32)
        })
        .collect()
}

/// A streaming resampler that pulls input frames on demand.
pub struct Resampler {
    /// Input frames per output frame, before drift.
    step: f64,
    /// Read-rate factor that follows a drifting clock.
    drift: f64,
    /// Input frames to pull before the next output frame, plus the fractional position between
    /// the two middle history frames.
    phase: f64,
    /// Input frames the kernel spans.
    taps: usize,
    /// `None` when the rates are equal and fixed.
    weights: Option<Vec<f32>>,
    /// The weights between two rows of [`Self::weights`], for the output frame at hand.
    blended: Vec<f32>,
    /// The last [`Self::taps`] input frames per channel, stored twice so they lie in order from
    /// [`Self::oldest`] in one slice.
    history: Vec<Vec<f32>>,
    /// Where the oldest input frame sits and the next one goes.
    oldest: usize,
    frame: Vec<f32>,
}

impl Resampler {
    /// A resampler between clocks that keep their rates; frames pass through at equal rates.
    #[must_use]
    pub fn fixed(channels: usize, input_rate: u32, output_rate: u32) -> Self {
        Self::new(channels, input_rate, output_rate, input_rate != output_rate)
    }

    /// A resampler that follows a drifting input clock (see [`Self::set_drift`]), so it filters
    /// at equal rates too.
    #[must_use]
    pub fn drifting(channels: usize, input_rate: u32, output_rate: u32) -> Self {
        Self::new(channels, input_rate, output_rate, true)
    }

    fn new(channels: usize, input_rate: u32, output_rate: u32, filtered: bool) -> Self {
        let step = f64::from(input_rate) / f64::from(output_rate);
        let cutoff = 0.5 * PASSBAND * step.recip().min(1.0);
        let half = (HALF as f64 * step.max(1.0)).ceil() as usize;
        let taps = 2 * half;

        Self {
            step,
            drift: 1.0,
            phase: taps as f64,
            taps,
            weights: filtered.then(|| table(half, cutoff)),
            blended: vec![0.0; taps],
            history: vec![vec![0.0; 2 * taps]; channels],
            oldest: 0,
            frame: vec![0.0; channels],
        }
    }

    /// Reads input `drift` times as fast as nominal from the next frame on.
    pub const fn set_drift(&mut self, drift: f64) {
        self.drift = drift;
    }

    /// Writes the next output frame to `out`, pulling input frames with `pull`.
    pub fn produce(&mut self, pull: &mut impl FnMut(&mut [f32]), out: &mut [f32]) {
        let Some(weights) = &self.weights else {
            pull(out);
            return;
        };

        let taps = self.taps;
        let whole = self.phase as usize;
        self.phase -= whole as f64;

        for _ in 0..whole {
            pull(&mut self.frame);

            let oldest = self.oldest;

            self.history
                .iter_mut()
                .zip(&self.frame)
                .for_each(|(history, sample)| {
                    history[oldest] = *sample;
                    history[oldest + taps] = *sample;
                });
            self.oldest = (oldest + 1) % taps;
        }

        let position = self.phase * PHASES as f64;
        let row = position as usize;
        let between = (position - row as f64) as f32;
        let (before, after) = weights[row * taps..(row + 2) * taps].split_at(taps);

        self.blended
            .iter_mut()
            .zip(before.iter().zip(after))
            .for_each(|(blended, (before, after))| {
                *blended = (after - before) * between + before;
            });

        let window = self.oldest..self.oldest + taps;

        out.iter_mut()
            .zip(&self.history)
            .for_each(|(sample, history)| {
                *sample = history[window.clone()]
                    .iter()
                    .zip(&self.blended)
                    .map(|(frame, weight)| frame * weight)
                    .sum();
            });

        self.phase += self.step * self.drift;
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use super::*;

    fn run(rates: (u32, u32), input: impl IntoIterator<Item = f32>, frames: usize) -> Vec<f32> {
        let mut resampler = Resampler::fixed(1, rates.0, rates.1);
        let mut source = input.into_iter().chain(iter::repeat(0.0));
        let mut pull = |frame: &mut [f32]| frame[0] = source.next().unwrap_or(0.0);

        (0..frames)
            .map(|_| {
                let mut out = [0.0];

                resampler.produce(&mut pull, &mut out);
                out[0]
            })
            .collect()
    }

    fn sine(rate: u32, frequency: f32) -> impl Iterator<Item = f32> {
        (0..).map(move |frame| (TAU * frequency * frame as f32 / rate as f32).sin())
    }

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0, |peak, it| peak.max(it.abs()))
    }

    #[test]
    fn equal_fixed_rates_copy_the_signal() {
        let input: Vec<f32> = (1..=8).map(|it| it as f32).collect();

        assert_eq!(run((48_000, 48_000), input.clone(), 8), input);
    }

    #[test]
    fn a_ramp_stays_a_ramp_at_another_rate() {
        let out = run((44_100, 48_000), (0..400).map(|it| it as f32), 300);

        for pair in out[40..].windows(2) {
            let slope = pair[1] - pair[0];

            assert!(
                (slope - 44_100.0 / 48_000.0).abs() < 1.0e-2,
                "slope {slope}"
            );
        }
    }

    #[test]
    fn a_tone_both_rates_hold_passes_whole() {
        let out = run((96_000, 48_000), sine(96_000, 1_000.0), 4_000);

        assert!((peak(&out[100..]) - 1.0).abs() < 0.01);
    }

    #[test]
    fn a_tone_above_the_lower_nyquist_is_filtered_out() {
        let out = run((96_000, 48_000), sine(96_000, 30_000.0), 4_000);

        assert!(peak(&out[100..]) < 1.0e-3, "peak {}", peak(&out[100..]));
    }

    #[test]
    fn a_tone_above_the_lower_nyquist_is_filtered_out_at_a_ratio_just_above_one() {
        let out = run((48_000, 44_100), sine(48_000, 24_000.0), 4_000);

        assert!(peak(&out[100..]) < 1.0e-3, "peak {}", peak(&out[100..]));
    }

    #[test]
    fn a_tone_just_above_the_lower_nyquist_is_filtered_out_at_four_times_the_rate() {
        let out = run((192_000, 48_000), sine(192_000, 26_000.0), 4_000);

        assert!(peak(&out[200..]) < 1.0e-3, "peak {}", peak(&out[200..]));
    }

    #[test]
    fn a_tone_both_rates_hold_passes_whole_at_four_times_the_rate() {
        let out = run((192_000, 48_000), sine(192_000, 1_000.0), 4_000);

        assert!((peak(&out[200..]) - 1.0).abs() < 0.01);
    }

    #[test]
    fn drift_reads_the_input_faster() {
        let mut resampler = Resampler::drifting(1, 48_000, 48_000);
        let mut pulled = 0;
        let mut pull = |frame: &mut [f32]| {
            pulled += 1;
            frame[0] = 0.0;
        };

        resampler.set_drift(1.01);
        (0..10_000).for_each(|_| resampler.produce(&mut pull, &mut [0.0]));

        assert!(
            (10_100..=10_100 + 2 * HALF).contains(&pulled),
            "pulled {pulled}"
        );
    }
}
