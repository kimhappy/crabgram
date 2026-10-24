use core::{marker::PhantomData, ops::Range};

use crate::{
    Controls, Plugin,
    anagram::{MAX_BPM, MIN_BPM},
    channels::{Inputs, Outputs},
    port::Layout,
};

fn slice(ports: &[*mut f32], indices: Range<u32>) -> &[*mut f32] {
    &ports[indices.start as usize..indices.end as usize]
}

pub(super) fn inputs<P: Plugin>(ports: &[*mut f32]) -> &[*mut f32] {
    slice(ports, Layout::of::<P>().audio_inputs())
}

pub(super) fn outputs<P: Plugin>(ports: &[*mut f32]) -> &[*mut f32] {
    slice(ports, Layout::of::<P>().audio_outputs())
}

fn controls<P: Plugin>(ports: &[*mut f32]) -> &[*mut f32] {
    slice(ports, Layout::of::<P>().control_ports())
}

/// The ports of an instance whose every port is connected, read and written during a run.
pub(super) struct Live<'a, P: Plugin> {
    ports: &'a [*mut f32],
    plugin: PhantomData<fn() -> P>,
}

impl<'a, P: Plugin> Live<'a, P> {
    /// # Safety
    ///
    /// Every port of `ports` must point to a live value of `f32`, valid for reads and writes for
    /// `'a`, and no two audio outputs may share a buffer.
    pub(super) const unsafe fn new(ports: &'a [*mut f32]) -> Self {
        Self {
            ports,
            plugin: PhantomData,
        }
    }

    /// # Safety
    ///
    /// Every audio port must point to [`FRAMES`](crate::anagram::FRAMES) live samples for `'a`, and
    /// no other port may overlap them.
    pub(super) unsafe fn audio(&self) -> (Inputs<'a>, Outputs<'a>) {
        // SAFETY: the caller upholds the contract, and `new` makes the outputs distinct.
        unsafe {
            (
                Inputs::new(inputs::<P>(self.ports)),
                Outputs::new(outputs::<P>(self.ports)),
            )
        }
    }

    const fn read(&self, port: u32) -> f32 {
        // SAFETY: `new` makes every port a live value, and `self` keeps it so.
        unsafe { self.ports[port as usize].read() }
    }

    /// Whether the reset trigger is held high.
    pub(super) fn reset_held(&self) -> bool {
        self.read(Layout::of::<P>().reset()) > 0.0
    }

    pub(super) fn enabled(&self) -> bool {
        self.read(Layout::of::<P>().enabled()) > 0.0
    }

    /// The tempo, clamped to `MIN_BPM..=MAX_BPM`; NaN becomes `MIN_BPM`.
    pub(super) const fn bpm(&self) -> f32 {
        self.read(Layout::of::<P>().tempo())
            .max(MIN_BPM)
            .min(MAX_BPM)
    }

    /// Copies every input control's port into its slot of `values`.
    pub(super) fn read_inputs(&self, values: &mut [f32]) {
        P::Controls::CONTROLS
            .iter()
            .zip(values)
            .zip(controls::<P>(self.ports))
            .filter(|((control, _), _)| !control.is_output())
            .for_each(|((_, value), port)| {
                // SAFETY: as in `read`.
                *value = unsafe { port.read() };
            });
    }

    /// Writes every output control's value to its port.
    pub(super) fn write_outputs(&self, values: &[f32]) {
        P::Controls::CONTROLS
            .iter()
            .zip(values)
            .zip(controls::<P>(self.ports))
            .filter(|((control, _), _)| control.is_output())
            .for_each(|((_, value), port)| {
                // SAFETY: as in `read`.
                unsafe { port.write(*value) };
            });
    }
}
