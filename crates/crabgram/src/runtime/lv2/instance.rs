use core::{ffi::c_void, mem, ptr};

use super::live::{self, Live};
use crate::{
    Context, Controls, Plugin,
    anagram::FRAMES,
    channels,
    context::Status,
    runtime::{
        host::{Host, Requests},
        licensing,
    },
};

#[derive(Clone, Copy)]
enum Connection {
    /// A port is unconnected or two audio outputs share a buffer, so runs do nothing.
    Unready,
    /// Every port is connected; `crossed` when an input is another channel's output.
    Ready { crossed: bool },
}

/// The reset trigger as the runs saw it.
#[derive(Default)]
struct Trigger {
    /// Whether the next block resets.
    due: bool,
    /// Whether the trigger was high at the last run.
    held: bool,
}

pub(super) struct Instance<P: Plugin>
where
    [(); P::PORTS]:,
{
    plugin: P,
    ports: [*mut f32; P::PORTS],
    /// `None` after a connection until the next run checks.
    connection: Option<Connection>,
    /// Inputs as the host set them, outputs as the plugin last wrote them.
    values: <P::Controls as Controls>::Values,
    /// One block per channel, to copy inputs that share buffers with outputs.
    scratch: Box<[[f32; FRAMES]]>,
    /// Whether the next block is the first after an activation.
    activated: bool,
    trigger: Trigger,
    requests: Requests,
    run_count: u32,
}

impl<P: Plugin> Instance<P>
where
    [(); P::PORTS]:,
{
    pub(super) fn new(host: Host) -> Self {
        Self {
            plugin: P::new(),
            ports: [ptr::null_mut(); P::PORTS],
            connection: None,
            values: P::Controls::default().to_ports(),
            scratch: vec![[0.0; FRAMES]; P::CHANNELS].into_boxed_slice(),
            activated: true,
            trigger: Trigger::default(),
            requests: Requests::new(host, P::Controls::CONTROLS.len()),
            run_count: 0,
        }
    }

    pub(super) fn connect(&mut self, port: u32, data: *mut c_void) {
        if let Some(slot) = self.ports.get_mut(port as usize) {
            let moved = *slot != data.cast();

            *slot = data.cast();

            if moved {
                self.connection = None;
            }
        }
    }

    fn connection(&self) -> Connection {
        let (inputs, outputs) = (
            live::inputs::<P>(&self.ports),
            live::outputs::<P>(&self.ports),
        );
        let distinct = outputs
            .iter()
            .enumerate()
            .all(|(channel, output)| !outputs[channel + 1..].contains(output));

        if distinct && self.ports.iter().all(|port| !port.is_null()) {
            Connection::Ready {
                crossed: channels::crossed(inputs, outputs),
            }
        } else {
            Connection::Unready
        }
    }

    pub(super) const fn activate(&mut self) {
        self.activated = true;
    }

    /// Runs the plugin on one block.
    ///
    /// # Safety
    ///
    /// As for [`Live::new`] and [`Live::audio`], with `frames` 0 or [`FRAMES`] and two audio
    /// buffers either the same or apart.
    pub(super) unsafe fn run(&mut self, frames: u32) {
        debug_assert!(
            frames == 0 || frames as usize == FRAMES,
            "this crate requires 0 or `FRAMES` frames, not {frames}"
        );

        let connection = self.connection.unwrap_or_else(|| self.connection());

        self.connection = Some(connection);

        let Connection::Ready { crossed } = connection else {
            return;
        };

        // SAFETY: a ready connection has every port connected and the outputs distinct, and `run`
        // requires every connected port to be live.
        let live = unsafe { Live::<P>::new(&self.ports) };

        let held = live.reset_held();
        let was_held = mem::replace(&mut self.trigger.held, held);
        let raised = held && !was_held;

        if frames == 0 {
            self.trigger.due |= raised;

            return;
        }

        let licensed_run = P::INFO.licensing().checks().then(|| {
            self.run_count = licensing::run_begin(self.run_count);
            self.run_count
        });
        let activated = mem::take(&mut self.activated);
        let reset_due = mem::take(&mut self.trigger.due);
        let reset = raised || reset_due || activated;

        if reset {
            self.requests.forget();
        }

        live.read_inputs(self.values.as_mut());
        self.requests.forget_moved(self.values.as_ref());

        // SAFETY: `run` requires live buffers of `FRAMES` samples, apart from the other ports,
        // and a ready connection has the outputs distinct.
        let (inputs, mut outputs) = unsafe { live.audio() };

        outputs.copy_from(&inputs, crossed, &mut self.scratch);

        let controls = P::Controls::from_ports(&self.values);
        // SAFETY: the port layout gives `P` one output per channel.
        let mut context = unsafe {
            Context::<P>::new(
                outputs.reborrow(),
                Status {
                    enabled: live.enabled(),
                    reset,
                    activated,
                    bpm: live.bpm(),
                },
                &self.requests,
                self.values.as_mut(),
            )
        };

        self.plugin.run(&controls, &mut context);

        live.write_outputs(self.values.as_ref());

        if let Some(run_count) = licensed_run {
            outputs.iter_mut().zip(0..).for_each(|(buffer, channel)| {
                licensing::run_silence(run_count, buffer, channel);
            });
        }
    }
}
