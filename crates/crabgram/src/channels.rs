//! The audio channels of a plugin, counted by [`Info::channels`](crate::Info::channels).

use crate::anagram::FRAMES;

/// `samples` with every one that is not finite silenced.
fn finite(samples: [f32; FRAMES]) -> [f32; FRAMES] {
    samples.map(|sample| if sample.is_finite() { sample } else { 0.0 })
}

/// Whether one of `inputs` is another channel's output buffer.
pub fn crossed(inputs: &[*mut f32], outputs: &[*mut f32]) -> bool {
    inputs
        .iter()
        .zip(outputs)
        .any(|(input, output)| input != output && outputs.contains(input))
}

/// The audio input buffers of a block.
#[derive(Clone, Copy)]
pub struct Inputs<'a>(&'a [*mut f32]);

impl<'a> Inputs<'a> {
    /// # Safety
    ///
    /// Every pointer must be to [`FRAMES`] live samples for `'a`.
    pub const unsafe fn new(pointers: &'a [*mut f32]) -> Self {
        Self(pointers)
    }
}

/// The audio output buffers of a block, one distinct buffer per channel.
pub struct Outputs<'a>(&'a [*mut f32]);

#[allow(
    clippy::needless_pass_by_ref_mut,
    reason = "the pointers are shared, so `&mut self` is what keeps the references exclusive"
)]
impl<'a> Outputs<'a> {
    /// # Safety
    ///
    /// The pointers must be distinct, each to [`FRAMES`] live samples for `'a`, apart from each
    /// other. Only the [`Inputs`] of [`copy_from`](Self::copy_from) may access them meanwhile,
    /// each input either an output's buffer or apart from every output.
    pub const unsafe fn new(pointers: &'a [*mut f32]) -> Self {
        Self(pointers)
    }

    /// The channel count.
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// The same buffers for a shorter time.
    pub const fn reborrow(&mut self) -> Outputs<'_> {
        Outputs(self.0)
    }

    /// Every channel, as an array.
    ///
    /// # Safety
    ///
    /// There must be `N` channels.
    pub unsafe fn exact<const N: usize>(&mut self) -> [&mut [f32; FRAMES]; N] {
        debug_assert_eq!(self.len(), N, "`N` is the channel count");

        // SAFETY: the caller makes `N` the channel count.
        let pointers: [*mut f32; N] = unsafe { self.0.try_into().unwrap_unchecked() };

        pointers.map(|pointer| {
            // SAFETY: `new` makes the pointers distinct and live, and `&mut self` keeps every
            // reference of this call exclusive.
            unsafe { &mut *pointer.cast() }
        })
    }

    /// Every channel, in order.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut [f32; FRAMES]> {
        self.0.iter().map(|&pointer| {
            // SAFETY: `new` makes the pointers distinct and live, and `&mut self` keeps every
            // reference of this call exclusive.
            unsafe { &mut *pointer.cast() }
        })
    }

    /// Copies each input into its output with non-finite samples silenced, through `scratch` when
    /// the buffers are [`crossed`].
    pub fn copy_from(&mut self, inputs: &Inputs<'_>, crossed: bool, scratch: &mut [[f32; FRAMES]]) {
        let outputs = self.0;
        let read = |channel: usize| {
            // SAFETY: `new` makes the input live, and a read creates no reference.
            finite(unsafe { inputs.0[channel].cast::<[f32; FRAMES]>().read() })
        };
        let write = |channel: usize, samples: [f32; FRAMES]| {
            // SAFETY: `new` makes the output live, and `&mut self` keeps it unaliased.
            unsafe { outputs[channel].cast::<[f32; FRAMES]>().write(samples) };
        };

        debug_assert_eq!(
            crossed,
            self::crossed(inputs.0, outputs),
            "`crossed` is that of `inputs`"
        );

        if crossed {
            (0..outputs.len()).for_each(|channel| scratch[channel] = read(channel));
            (0..outputs.len()).for_each(|channel| write(channel, scratch[channel]));
        } else {
            (0..outputs.len()).for_each(|channel| write(channel, read(channel)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FRAMES, Inputs, Outputs};

    type Buffer = [f32; FRAMES];

    fn ramp(offset: f32) -> Buffer {
        core::array::from_fn(|index| offset + index as f32)
    }

    fn pointers<const N: usize>(buffers: &mut [Buffer; N]) -> [*mut f32; N] {
        buffers.each_mut().map(|buffer| buffer.as_mut_ptr())
    }

    /// Copies `inputs` to `outputs`, all among `buffers`.
    fn copy<const N: usize>(buffers: &mut [Buffer; N], inputs: &[usize], outputs: &[usize]) {
        let pointers = pointers(buffers);
        let pick = |channels: &[usize]| channels.iter().map(|&it| pointers[it]).collect::<Vec<_>>();
        let (inputs, outputs) = (pick(inputs), pick(outputs));
        let mut scratch = vec![[0.0; FRAMES]; outputs.len()];

        // SAFETY: the pointers are to live buffers of `FRAMES` samples, the outputs distinct.
        let (inputs, mut outputs) = unsafe { (Inputs::new(&inputs), Outputs::new(&outputs)) };
        outputs.copy_from(&inputs, super::crossed(inputs.0, outputs.0), &mut scratch);
    }

    #[test]
    fn disjoint_buffers_are_copied() {
        let mut buffers = [ramp(0.0), ramp(100.0), [0.0; FRAMES], [0.0; FRAMES]];

        copy(&mut buffers, &[0, 1], &[2, 3]);

        assert_eq!(buffers[2..], [ramp(0.0), ramp(100.0)]);
    }

    #[test]
    fn an_input_that_is_its_output_stays() {
        let mut buffers = [ramp(5.0)];

        copy(&mut buffers, &[0], &[0]);

        assert_eq!(buffers, [ramp(5.0)]);
    }

    #[test]
    fn swapped_buffers_are_read_before_they_are_written() {
        let mut buffers = [ramp(0.0), ramp(100.0)];

        copy(&mut buffers, &[0, 1], &[1, 0]);

        assert_eq!(buffers, [ramp(100.0), ramp(0.0)]);
    }

    #[test]
    fn a_chain_of_shared_buffers_keeps_every_input_intact() {
        let mut buffers = [ramp(0.0), ramp(100.0), ramp(200.0)];

        copy(&mut buffers, &[0, 1], &[1, 2]);

        assert_eq!(buffers, [ramp(0.0), ramp(0.0), ramp(100.0)]);
    }

    #[test]
    fn a_sample_that_is_not_finite_becomes_silence() {
        let mut buffers = [ramp(0.0), ramp(100.0)];

        buffers[0][3] = f32::NAN;
        buffers[1][5] = f32::INFINITY;
        buffers[1][7] = f32::NEG_INFINITY;

        let mut expected = [ramp(0.0), ramp(100.0)];

        expected[0][3] = 0.0;
        expected[1][5] = 0.0;
        expected[1][7] = 0.0;

        copy(&mut buffers, &[0, 1], &[0, 1]);

        assert_eq!(buffers, expected);
    }

    #[test]
    fn a_copied_sample_that_is_not_finite_becomes_silence() {
        let mut buffers = [ramp(0.0), [0.0; FRAMES]];

        buffers[0][0] = f32::NAN;
        copy(&mut buffers, &[0], &[1]);

        assert_eq!(buffers[1][0], 0.0);
        assert_eq!(buffers[1][1..], ramp(0.0)[1..]);
    }

    #[test]
    fn buffers_are_the_outputs_in_order() {
        let mut buffers = [ramp(0.0), ramp(100.0)];
        let pointers = pointers(&mut buffers);

        // SAFETY: two distinct live buffers.
        let mut outputs = unsafe { Outputs::new(&pointers) };
        // SAFETY: there are two channels.
        let [left, right] = unsafe { outputs.exact() };

        left[0] = -1.0;
        right[0] = -2.0;

        assert_eq!(outputs.len(), 2);
        assert_eq!((buffers[0][0], buffers[1][0]), (-1.0, -2.0));
    }

    #[test]
    fn iterating_visits_each_channel_once() {
        let mut buffers = [ramp(0.0), ramp(100.0)];
        let pointers = pointers(&mut buffers);

        // SAFETY: two distinct live buffers.
        let mut outputs = unsafe { Outputs::new(&pointers) };

        outputs
            .iter_mut()
            .enumerate()
            .for_each(|(channel, buffer)| buffer.fill(channel as f32));

        assert_eq!(buffers, [[0.0; FRAMES], [1.0; FRAMES]]);
    }
}
