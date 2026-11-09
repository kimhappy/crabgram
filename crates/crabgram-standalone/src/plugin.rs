//! The plugin, loaded and run block by block.

use core::{
    cell::Cell,
    ffi::{CStr, c_void},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU32, Ordering},
    },
};

use crabgram::{
    Control, ControlState,
    anagram::{FRAMES, MIN_BPM},
    lv2::{
        ConnectPortFn, Descriptor, Handle, HandleFn, HostCalls, HostFeatures, HostRefusal,
        InstantiateFn, RunFn,
    },
    port::Layout,
};
use crabgram_styling::ui::Model;

use crate::audio::SAMPLE_RATE;

type DescriptorFn = unsafe extern "C" fn(index: u32) -> *const Descriptor;

/// Why the library cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("cannot load {}", path.display())]
    Load {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },
    #[error("{} is no LV2 plugin", path.display())]
    NoEntryPoint {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },
    #[error("{} holds no plugin", path.display())]
    NoPlugin { path: PathBuf },
    #[error(
        "{}: the plugin is {found}, but {} describes {}: bundle it again",
        path.display(),
        crabgram_styling::ui::FILE,
        if described.is_empty() { "no plugin" } else { described }
    )]
    OtherPlugin {
        path: PathBuf,
        found: String,
        described: String,
    },
}

/// Why the plugin does not start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("the bundle path holds a NUL byte")]
    NulInBundlePath,
    #[error("the plugin has no instantiate")]
    NoInstantiate,
    #[error("the plugin failed to instantiate")]
    Instantiate,
}

/// The entry points of a plugin, copied out of its descriptor.
#[derive(Clone, Copy)]
struct EntryPoints {
    instantiate: Option<InstantiateFn>,
    connect_port: Option<ConnectPortFn>,
    activate: Option<HandleFn>,
    run: Option<RunFn>,
    deactivate: Option<HandleFn>,
    cleanup: Option<HandleFn>,
}

impl EntryPoints {
    const fn of(descriptor: &Descriptor) -> Self {
        Self {
            instantiate: descriptor.instantiate,
            connect_port: descriptor.connect_port,
            activate: descriptor.activate,
            run: descriptor.run,
            deactivate: descriptor.deactivate,
            cleanup: descriptor.cleanup,
        }
    }
}

/// The plugin of a shared library, kept loaded by every [`Instance`].
pub struct Library {
    /// The first plugin's descriptor, which lives as long as the library stays loaded.
    descriptor: NonNull<Descriptor>,
    entry_points: EntryPoints,
    /// The library holding the descriptor; none for one linked into this program.
    _loaded: Option<libloading::Library>,
}

// SAFETY: `_loaded` keeps the immutable descriptor alive, and its entry points take any thread.
unsafe impl Send for Library {}

// SAFETY: as for `Send`; `Library` never changes after it opens.
unsafe impl Sync for Library {}

impl Library {
    /// Loads the plugin at `path`, which must be the plugin `uri` names.
    pub fn open(path: &Path, uri: &str) -> Result<Self, LoadError> {
        // SAFETY: loading runs the library's initializers, which are sound for a crabgram build;
        // this assumes the path is one.
        let library =
            unsafe { libloading::Library::new(path) }.map_err(|source| LoadError::Load {
                path: path.to_owned(),
                source,
            })?;
        // SAFETY: `lv2_descriptor` is the LV2 entry point of a crabgram build, with this signature.
        let entry =
            unsafe { library.get::<DescriptorFn>(b"lv2_descriptor\0") }.map_err(|source| {
                LoadError::NoEntryPoint {
                    path: path.to_owned(),
                    source,
                }
            })?;
        // SAFETY: the entry point answers the first plugin's descriptor, or null for none.
        let descriptor =
            NonNull::new(unsafe { entry(0) }.cast_mut()).ok_or_else(|| LoadError::NoPlugin {
                path: path.to_owned(),
            })?;

        // SAFETY: a descriptor of a crabgram build is an aligned, readable `Descriptor` with a
        // NUL-terminated URI for as long as `library` stays loaded, which it does in `Self`.
        unsafe { Self::of(descriptor, uri, path, Some(library)) }
    }

    /// The plugin of a descriptor that lives forever, if it is the one `uri` names.
    #[cfg(test)]
    fn linked(descriptor: &'static Descriptor, uri: &str) -> Result<Self, LoadError> {
        // SAFETY: a linked descriptor is static and holds a NUL-terminated URI.
        unsafe { Self::of(NonNull::from(descriptor), uri, Path::new("linked"), None) }
    }

    /// The plugin of `descriptor`, kept alive by `loaded`, if it is the one `uri` names.
    ///
    /// # Safety
    ///
    /// `descriptor` must point to a readable `Descriptor` with a NUL-terminated URI for as long as
    /// `loaded` lives, or forever without it.
    unsafe fn of(
        descriptor: NonNull<Descriptor>,
        uri: &str,
        path: &Path,
        loaded: Option<libloading::Library>,
    ) -> Result<Self, LoadError> {
        // SAFETY: the caller passes a live descriptor.
        let read = unsafe { descriptor.as_ref() };
        // SAFETY: the caller passes a NUL-terminated URI.
        let found = unsafe { CStr::from_ptr(read.uri) };

        if found.to_bytes() == uri.as_bytes() {
            Ok(Self {
                descriptor,
                entry_points: EntryPoints::of(read),
                _loaded: loaded,
            })
        } else {
            Err(LoadError::OtherPlugin {
                path: path.to_owned(),
                found: found.to_string_lossy().into_owned(),
                described: uri.to_owned(),
            })
        }
    }

    /// The descriptor `instantiate` is given back.
    const fn descriptor(&self) -> *const Descriptor {
        self.descriptor.as_ptr()
    }
}

/// What a port is to the screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortRole {
    /// A control the screen sets and the plugin may ask to change.
    Input,
    /// A control the plugin writes for the screen.
    Output,
    /// An audio, bypass, reset or tempo port, which is no control of the model.
    Builtin,
}

impl PortRole {
    const fn of(control: Option<&Control>) -> Self {
        match control {
            Some(control) if control.is_output() => Self::Output,
            Some(_) => Self::Input,
            None => Self::Builtin,
        }
    }
}

/// One port's value, written by the plugin or the screen, and the state the plugin put it in.
struct SharedPort {
    bits: AtomicU32,
    role: PortRole,
    state: AtomicU32,
}

impl SharedPort {
    fn load(&self) -> f32 {
        f32::from_bits(self.bits.load(Ordering::Relaxed))
    }

    fn store(&self, value: f32) {
        self.bits.store(value.to_bits(), Ordering::Relaxed);
    }
}

/// The index of a port in the plugin's layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortIndex(pub u32);

/// A value per port, shared with the audio thread; audio ports' go unused. The plugin may only
/// change a control's state and ask an input control to change.
pub struct Shared {
    ports: Vec<SharedPort>,
}

impl Shared {
    /// Every port at its default, the block on.
    #[must_use]
    pub fn new(model: &Model) -> Self {
        let layout = model.layout();
        let control = |port| {
            layout
                .position(port)
                .map(|position| &model.controls()[position as usize])
        };
        let initial = |port| match control(port) {
            Some(control) => control.default_value(),
            None if port == layout.enabled() => 1.0,
            None if port == layout.tempo() => MIN_BPM,
            None => 0.0,
        };

        Self {
            ports: (0..layout.count())
                .map(|port| SharedPort {
                    bits: AtomicU32::new(initial(port).to_bits()),
                    role: PortRole::of(control(port)),
                    state: AtomicU32::new(ControlState::Normal as u32),
                })
                .collect(),
        }
    }

    /// Port `index`, which the screen only asks for within the layout.
    fn port(&self, PortIndex(index): PortIndex) -> Option<&SharedPort> {
        debug_assert!(
            (index as usize) < self.ports.len(),
            "port {index} is past the layout"
        );
        self.ports.get(index as usize)
    }

    pub fn set(&self, index: PortIndex, value: f32) {
        if let Some(port) = self.port(index) {
            port.store(value);
        }
    }

    #[must_use]
    pub fn get(&self, index: PortIndex) -> f32 {
        self.port(index).map_or(0.0, SharedPort::load)
    }

    /// Puts every control back in its normal state, as a new instance has asked for none.
    fn release_states(&self) {
        self.ports.iter().for_each(|port| {
            port.state
                .store(ControlState::Normal as u32, Ordering::Relaxed);
        });
    }

    /// The state the plugin last put port `index` in.
    #[must_use]
    pub fn state(&self, index: PortIndex) -> ControlState {
        self.port(index)
            .and_then(|port| ControlState::from_number(port.state.load(Ordering::Relaxed)))
            .unwrap_or(ControlState::Normal)
    }
}

/// The host side of the plugin's calls: records control states for the screen and sets input
/// controls.
impl HostCalls for Shared {
    fn update_state(&self, index: u32, state: ControlState) -> Result<(), HostRefusal> {
        let port = self
            .ports
            .get(index as usize)
            .filter(|port| port.role != PortRole::Builtin)
            .ok_or(HostRefusal::InvalidIndex)?;

        port.state.store(state as u32, Ordering::Relaxed);
        Ok(())
    }

    fn request_change(&self, index: u32, value: f32) -> Result<(), HostRefusal> {
        let port = self
            .ports
            .get(index as usize)
            .filter(|port| port.role == PortRole::Input)
            .ok_or(HostRefusal::InvalidIndex)?;

        port.store(value);
        Ok(())
    }
}

/// One port's buffer: a block of audio, or a control's value in its first sample. The instance
/// writes through a pointer to it, so it is only ever reached through shared references.
type PortBuffer = [Cell<f32>; FRAMES];

fn silence() -> PortBuffer {
    core::array::from_fn(|_| Cell::new(0.0))
}

pub struct Instance {
    library: Arc<Library>,
    handle: Handle,
    /// Where each port sits.
    layout: Layout,
    /// One buffer per port, never resized: the instance holds pointers into them.
    ports: Vec<PortBuffer>,
    /// The ports past the audio ports that take the screen's value before each block.
    pulled: Vec<usize>,
    /// The output controls, copied out for the screen after each block.
    pushed: Vec<usize>,
    /// The instance calls into `Shared` through these.
    features: HostFeatures<Shared>,
}

// SAFETY: only the thread owning `Instance` uses the instance behind `handle`, and the buffers it
// points into move along with `Instance`.
unsafe impl Send for Instance {}

impl Instance {
    /// Instantiates the plugin of `host` at the rate of the plugin API, connects every port and
    /// activates it.
    pub fn new(host: &Host) -> Result<Self, StartError> {
        let library = Arc::clone(&host.library);
        let layout = host.model.layout();
        let bundle_with_separator =
            std::ffi::CString::new(host.bundle.join("").as_os_str().as_encoded_bytes())
                .map_err(|_| StartError::NulInBundlePath)?;
        let instantiate = library
            .entry_points
            .instantiate
            .ok_or(StartError::NoInstantiate)?;
        let ports = vec![silence(); host.shared.ports.len()];

        host.shared.release_states();

        let features = HostFeatures::new(Arc::clone(&host.shared));
        // SAFETY: the arguments are what LV2 gives `instantiate`; the features outlive the instance.
        let handle = unsafe {
            instantiate(
                library.descriptor(),
                f64::from(SAMPLE_RATE),
                bundle_with_separator.as_ptr(),
                features.list(),
            )
        };

        if handle.is_null() {
            return Err(StartError::Instantiate);
        }

        let shared = &host.shared.ports;
        let value_ports = |role_kept: fn(PortRole) -> bool| {
            (layout.enabled() as usize..shared.len())
                .filter(|&port| role_kept(shared[port].role))
                .collect()
        };
        let instance = Self {
            library,
            handle,
            layout,
            ports,
            pulled: value_ports(|role| role != PortRole::Output),
            pushed: value_ports(|role| role == PortRole::Output),
            features,
        };

        instance.connect();
        instance.pull();

        if let Some(activate) = instance.library.entry_points.activate {
            // SAFETY: `handle` is the instance just made, every port connected.
            unsafe { activate(handle) };
        }

        Ok(instance)
    }

    fn connect(&self) {
        let Some(connect) = self.library.entry_points.connect_port else {
            return;
        };

        for (port, buffer) in self.ports.iter().enumerate() {
            let data = buffer.as_ptr().cast_mut().cast::<c_void>();

            // SAFETY: `handle` is live, and `data` points at `FRAMES` cells, which the instance may
            // write through; `ports` stays put until `drop` cleans the instance up.
            unsafe { connect(self.handle, port as u32, data) };
        }
    }

    /// Copies the screen's values into the ports it sets.
    fn pull(&self) {
        let shared = &self.features.host().ports;

        self.pulled
            .iter()
            .for_each(|&port| self.ports[port][0].set(shared[port].load()));
    }

    /// Copies the output controls out for the screen.
    fn push(&self) {
        let shared = &self.features.host().ports;

        self.pushed
            .iter()
            .for_each(|&port| shared[port].store(self.ports[port][0].get()));
    }

    #[must_use]
    pub const fn channels(&self) -> usize {
        self.layout.channels() as usize
    }

    #[must_use]
    pub fn input(&self, channel: usize) -> &[Cell<f32>] {
        &self.ports[self.layout.audio_in(channel as u32) as usize]
    }

    #[must_use]
    pub fn output(&self, channel: usize) -> &[Cell<f32>] {
        &self.ports[self.layout.audio_out(channel as u32) as usize]
    }

    pub fn run(&mut self) {
        self.pull();

        if let Some(run) = self.library.entry_points.run {
            // SAFETY: `handle` is live, every port connected to a `FRAMES` buffer.
            unsafe { run(self.handle, FRAMES as u32) };
        }

        self.push();
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let entry_points = self.library.entry_points;

        if let Some(deactivate) = entry_points.deactivate {
            // SAFETY: `handle` is the live instance this owns.
            unsafe { deactivate(self.handle) };
        }
        if let Some(cleanup) = entry_points.cleanup {
            // SAFETY: `handle` is the live instance this owns, never used again.
            unsafe { cleanup(self.handle) };
        }
    }
}

/// What starts the plugin.
pub struct Host {
    model: Model,
    shared: Arc<Shared>,
    library: Arc<Library>,
    bundle: PathBuf,
}

impl Host {
    #[must_use]
    pub fn new(model: Model, library: Library, bundle: PathBuf) -> Self {
        Self {
            shared: Arc::new(Shared::new(&model)),
            library: Arc::new(library),
            model,
            bundle,
        }
    }

    #[must_use]
    pub const fn model(&self) -> &Model {
        &self.model
    }

    #[must_use]
    pub fn shared(&self) -> &Shared {
        &self.shared
    }
}

/// Keeps the instance across audio restarts.
#[derive(Clone, Default)]
pub struct Shelf(Arc<Mutex<Option<Instance>>>);

impl Shelf {
    /// The instance put back last, else a new one of `host`, lent until it is dropped.
    ///
    /// # Errors
    ///
    /// When the shelf is empty and the plugin cannot be instantiated.
    pub fn lend(&self, host: &Host) -> Result<Lent, StartError> {
        let kept = self.0.lock().unwrap_or_else(PoisonError::into_inner).take();
        let instance = kept.map_or_else(|| Instance::new(host), Ok)?;

        Ok(Lent {
            instance: Some(instance),
            shelf: self.clone(),
        })
    }
}

/// An instance off its [`Shelf`], put back there when dropped.
pub struct Lent {
    /// Taken only when dropped.
    instance: Option<Instance>,
    shelf: Shelf,
}

const LENT: &str = "a lent instance is there until it is dropped";

impl Deref for Lent {
    type Target = Instance;

    fn deref(&self) -> &Instance {
        self.instance.as_ref().expect(LENT)
    }
}

impl DerefMut for Lent {
    fn deref_mut(&mut self) -> &mut Instance {
        self.instance.as_mut().expect(LENT)
    }
}

impl Drop for Lent {
    fn drop(&mut self) {
        *self.shelf.0.lock().unwrap_or_else(PoisonError::into_inner) = self.instance.take();
    }
}

#[cfg(test)]
pub mod tests {
    use core::{cell::Cell, num::NonZeroU32};
    use std::{path::PathBuf, sync::Mutex, thread};

    use crabgram::{
        Context, Control, ControlState, Controls, Info, Plugin,
        anagram::FRAMES,
        lv2::{Descriptor, HostCalls, HostRefusal},
        port::Layout,
    };
    use crabgram_styling::{BlockSettings, ui::Model};

    use super::{Host, Instance, Library, LoadError, PortIndex, Shared, Shelf};

    #[derive(Controls)]
    struct Params {
        rate: f32,
        #[control(output)]
        level: f32,
    }

    /// Writes twice its rate to its level, and passes the audio through. With a `CHANNELS` other
    /// than the one a stale `ui.json` describes, it lays its ports out unlike the model.
    struct Doubler<const CHANNELS: u32>;

    impl<const CHANNELS: u32> Plugin for Doubler<CHANNELS> {
        type Controls = Params;

        const INFO: Info = crabgram::info! {
            uri: c"urn:crabgram:test:doubler",
            name: "Doubler",
            description: "Doubles its rate.",
            abbreviation: "DBL",
            category: Utility,
            channels: CHANNELS,
        };

        fn new() -> Self {
            Self
        }

        fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
            context.write(Params::LEVEL, 2.0 * controls.rate);
        }
    }

    static DOUBLER: Descriptor = crabgram::lv2::descriptor::<Doubler<1>>();
    static STEREO_DOUBLER: Descriptor = crabgram::lv2::descriptor::<Doubler<2>>();

    /// The name of each thread a [`Recorder`] was dropped on.
    static DROPPED_ON: Mutex<Vec<Option<String>>> = Mutex::new(Vec::new());

    struct Recorder;

    impl Plugin for Recorder {
        type Controls = Params;

        const INFO: Info = crabgram::info! {
            uri: c"urn:crabgram:test:recorder",
            name: "Recorder",
            description: "Records the thread it is dropped on.",
            abbreviation: "REC",
            category: Utility,
            channels: 1,
        };

        fn new() -> Self {
            Self
        }

        fn run(&mut self, _: &Params, _: &mut Context<'_, Self>) {}
    }

    impl Drop for Recorder {
        fn drop(&mut self) {
            DROPPED_ON
                .lock()
                .unwrap()
                .push(thread::current().name().map(str::to_owned));
        }
    }

    static RECORDER: Descriptor = crabgram::lv2::descriptor::<Recorder>();

    fn model() -> Model {
        model_of(Params::CONTROLS.to_vec())
    }

    fn model_of(controls: Vec<Control>) -> Model {
        Model::new(
            Doubler::<1>::INFO.uri(),
            "Probe",
            NonZeroU32::MIN,
            controls,
            BlockSettings::default(),
        )
    }

    fn on_thread(name: &str, work: impl FnOnce() + Send + 'static) {
        thread::Builder::new()
            .name(name.to_owned())
            .spawn(work)
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn an_instance_back_from_the_audio_thread_cleans_up_where_its_shelf_drops() {
        let model = Model::new(
            Recorder::INFO.uri(),
            "Recorder",
            NonZeroU32::MIN,
            Params::CONTROLS.to_vec(),
            BlockSettings::default(),
        );
        let host = host(model, &RECORDER);
        let shelf = Shelf::default();
        let lent = shelf.lend(&host).unwrap();

        on_thread("audio", move || drop(lent));
        assert!(DROPPED_ON.lock().unwrap().is_empty());

        on_thread("worker", move || drop(shelf));
        assert_eq!(*DROPPED_ON.lock().unwrap(), [Some("worker".to_owned())]);
    }

    fn doubler(uri: &str) -> Result<Library, LoadError> {
        Library::linked(&DOUBLER, uri)
    }

    /// A host of a plugin that passes its one channel through.
    pub fn passing_host() -> Host {
        host(model(), &DOUBLER)
    }

    fn host(model: Model, descriptor: &'static Descriptor) -> Host {
        let library = Library::linked(descriptor, model.uri()).unwrap();

        Host::new(model, library, PathBuf::from("probe"))
    }

    fn block(cells: &[Cell<f32>]) -> Vec<f32> {
        cells.iter().map(Cell::get).collect()
    }

    fn ramp() -> [f32; FRAMES] {
        core::array::from_fn(|frame| frame as f32)
    }

    fn feed(instance: &Instance, samples: &[f32]) {
        instance
            .input(0)
            .iter()
            .zip(samples)
            .for_each(|(cell, sample)| cell.set(*sample));
    }

    #[test]
    fn a_running_plugin_reads_its_inputs_and_writes_its_outputs() {
        let host = host(model(), &DOUBLER);
        let mut instance = Instance::new(&host).unwrap();
        let layout = host.model.layout();
        let (rate, level) = (PortIndex(layout.control(0)), PortIndex(layout.control(1)));
        let ramp = ramp();

        host.shared.set(rate, 0.25);
        feed(&instance, &ramp);
        instance.run();

        assert_eq!(host.shared.get(level), 0.5);
        assert_eq!(block(instance.output(0)), ramp);
    }

    #[test]
    fn a_plugin_laid_out_unlike_its_model_writes_only_into_its_own_buffers() {
        let model = model_of([Params::CONTROLS, Params::CONTROLS].concat());
        let host = host(model, &STEREO_DOUBLER);
        let mut instance = Instance::new(&host).unwrap();
        let ramp = ramp();
        let plugin_output = Layout::of::<Doubler<2>>().audio_out(0) as usize;

        assert_eq!(
            Layout::of::<Doubler<2>>().count(),
            host.model.layout().count()
        );

        feed(&instance, &ramp);
        instance.run();

        assert_eq!(
            block(&instance.ports[plugin_output]),
            ramp,
            "the plugin's first output is the model's bypass port, a whole block of its own"
        );
        assert!(
            instance
                .ports
                .as_flattened()
                .iter()
                .all(|cell| cell.get().is_finite())
        );
    }

    #[test]
    fn a_library_of_another_plugin_is_refused() {
        let refusal = |uri| doubler(uri).err().unwrap().to_string();

        assert_eq!(
            refusal("urn:crabgram:test:other"),
            "linked: the plugin is urn:crabgram:test:doubler, but ui.json describes \
             urn:crabgram:test:other: bundle it again"
        );
        assert_eq!(
            refusal(""),
            "linked: the plugin is urn:crabgram:test:doubler, but ui.json describes no plugin: \
             bundle it again"
        );
    }

    #[test]
    fn a_state_the_plugin_sets_reaches_the_screen() {
        let model = model();
        let shared = Shared::new(&model);
        let rate = model.layout().control(0);
        let refused = Err(HostRefusal::InvalidIndex);

        assert_eq!(shared.update_state(rate, ControlState::Blocked), Ok(()));
        assert_eq!(shared.state(PortIndex(rate)), ControlState::Blocked);
        assert_eq!(shared.update_state(999, ControlState::Inactive), refused);
        assert_eq!(
            shared.update_state(model.layout().enabled(), ControlState::Inactive),
            refused
        );
        assert_eq!(shared.state(PortIndex(rate)), ControlState::Blocked);
    }

    #[test]
    fn a_lent_instance_comes_back_to_its_shelf() {
        let host = host(model(), &DOUBLER);
        let shelf = Shelf::default();
        let rate = host.model.layout().control(0);
        let first = shelf.lend(&host).unwrap().handle;

        assert_eq!(
            host.shared.update_state(rate, ControlState::Blocked),
            Ok(())
        );
        assert_eq!(shelf.lend(&host).unwrap().handle, first);
        assert_eq!(host.shared.state(PortIndex(rate)), ControlState::Blocked);
    }

    #[test]
    fn a_new_instance_starts_with_every_control_normal() {
        let host = host(model(), &DOUBLER);
        let rate = host.model.layout().control(0);

        assert_eq!(
            host.shared.update_state(rate, ControlState::Blocked),
            Ok(())
        );

        let _instance = Instance::new(&host).unwrap();

        assert_eq!(host.shared.state(PortIndex(rate)), ControlState::Normal);
    }

    #[test]
    fn a_change_the_plugin_requests_sets_an_input_control_but_nothing_else() {
        let model = model();
        let shared = Shared::new(&model);
        let layout = model.layout();
        let (rate, level) = (layout.control(0), layout.control(1));
        let refused = Err(HostRefusal::InvalidIndex);

        assert_eq!(shared.request_change(rate, 0.25), Ok(()));
        assert_eq!(shared.get(PortIndex(rate)), 0.25);
        assert_eq!(shared.request_change(level, 0.5), refused);
        assert_eq!(shared.request_change(999, 0.5), refused);
        assert_eq!(shared.get(PortIndex(level)), 0.0);

        let others = [
            layout.audio_in(0),
            layout.enabled(),
            layout.reset(),
            layout.tempo(),
        ];

        assert_eq!(
            others.map(|port| shared.request_change(port, 0.5)),
            [refused; 4]
        );
    }
}
