//! The standalone app of every native bundle: the plugin on the system's audio devices, under the
//! block settings preview.
//!
//! The bundle command sets `CRABGRAM_PLUGIN`, the library to load, and `CRABGRAM_PACKAGE` and
//! `CRABGRAM_VERSION`, which name the app; the app finds the library, `ui.json` and the styling
//! images next to its executable.
#![feature(error_reporter)]
#![cfg_attr(test, feature(generic_const_exprs))]

mod audio;
mod compose;
mod plugin;
mod render;
mod strip;
mod window;

use core::time::Duration;
use std::{env, error::Report, fs, io, path::PathBuf, process::ExitCode, sync::Arc, time::Instant};

use audio::{Audio, AudioError, Dir, EngineStatus, PerDir, Reply};
use compose::Composer;
use crabgram_styling::{
    host,
    ui::{self, Model},
};
use plugin::{Host, Library, LoadError, PortIndex};
use render::{Images, Scene, State};
use strip::{Change, ContentPoint, Strip};
use tiny_skia::Pixmap;
use window::{Event, Surface, Wake, WindowError};
use winit::event_loop::{EventLoop, EventLoopProxy};

/// Why the app did not start or stopped.
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("cannot find the executable")]
    CurrentExe {
        #[source]
        source: io::Error,
    },
    #[error("the executable has no directory")]
    NoDirectory,
    #[error("cannot read {}", path.display())]
    ReadModelFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot parse {}", path.display())]
    ParseModel {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Load(#[from] LoadError),
    #[error("cannot start the audio thread")]
    AudioThread {
        #[source]
        source: io::Error,
    },
    #[error("the window's event loop failed")]
    EventLoop {
        #[source]
        source: winit::error::EventLoopError,
    },
    #[error(transparent)]
    Window(#[from] WindowError),
}

const PLUGIN: &str = env!(
    "CRABGRAM_PLUGIN",
    "CRABGRAM_PLUGIN is unset: build the standalone app with the bundle command of crabgram-build"
);
const PACKAGE: &str = env!(
    "CRABGRAM_PACKAGE",
    "CRABGRAM_PACKAGE is unset: build the standalone app with the bundle command of crabgram-build"
);
const VERSION: &str = env!(
    "CRABGRAM_VERSION",
    "CRABGRAM_VERSION is unset: build the standalone app with the bundle command of crabgram-build"
);
/// What the status puts before an audio failure.
const RETRY: &str = "pick the device again to retry";
/// Longest animation step, so a restart after a pause does not skip ahead.
const MAX_STEP: f32 = 0.05;
/// Time between frames while the screen moves without input.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// Time between looks at the plugin while no output widget shows.
const IDLE_INTERVAL: Duration = Duration::from_millis(100);

/// The window's size in screen pixels, unscaled: the strip above the screen.
const CONTENT: (u32, u32) = (host::SCREEN.0, host::SCREEN.1 + strip::HEIGHT as u32);

struct App {
    host: Arc<Host>,
    state: State,
    strip: Strip,
    audio: Audio,
    /// The engine on the audio thread, once it started.
    running: Option<EngineStatus>,
    /// Whether the status shows the running engine's fault.
    fault_shown: bool,
    images: Images,
    composer: Composer,
    last: Instant,
}

impl App {
    /// Restarts the audio on the strip's settings, reading the devices where `reread` holds again.
    fn restart(&mut self, reread: PerDir<bool>) {
        self.running = None;
        self.fault_shown = false;
        self.strip.set_status("starting the audio");
        self.audio.start(self.strip.io().clone(), reread);
    }

    /// Shows what the audio thread did; whether the strip changed.
    fn take_audio_replies(&mut self) -> bool {
        let revision = self.strip.revision();
        let mut started = None;
        let mut relisted = false;

        for reply in self.audio.replies() {
            match reply {
                Reply::Fitted { caps, io, .. } => self.strip.show_fitted(caps, io),
                Reply::Started { result, .. } => started = Some(result),
                Reply::Listed(names) => relisted |= self.strip.relist(names),
            }
        }

        if let Some(result) = started {
            self.started(result);
        }

        relisted || self.strip.revision() != revision
    }

    fn started(&mut self, result: Result<EngineStatus, AudioError>) {
        match result {
            Ok(running) => {
                let io = self.strip.io();
                let format = format!(
                    "plugin {} Hz, {} frames | in {} | out {}",
                    audio::SAMPLE_RATE,
                    crabgram::anagram::FRAMES,
                    io.input
                        .as_ref()
                        .map_or_else(|| "off".to_owned(), ToString::to_string),
                    io.output
                );

                self.strip.set_status(
                    running
                        .input_error()
                        .map_or(format, |error| format!("no input: {}", Report::new(error))),
                );
                self.running = Some(running);
            }
            Err(error) => {
                let report = Report::new(error).to_string();

                eprintln!("audio: {report}");
                self.strip.set_status(format!("{RETRY}: {report}"));
            }
        }
    }

    /// Copies the plugin's ports into the screen and the strip; whether the screen shows a change.
    fn take_plugin_changes(&mut self) -> bool {
        let shared = self.host.shared();

        self.strip
            .show_tempo(shared.get(PortIndex(self.host.model().layout().tempo())));
        self.state.sync(self.host.model(), |port| {
            (shared.get(port), shared.state(port))
        })
    }

    /// Whether the audio or just its input failed, so picking the same setting retries it.
    fn audio_failed(&self) -> bool {
        !self.audio.is_busy()
            && self
                .running
                .as_ref()
                .is_none_or(|running| running.input_error().is_some() || running.fault().is_some())
    }

    /// Prints and shows a running stream's error; whether the status changed.
    fn show_fault(&mut self) -> bool {
        let unshown = self
            .running
            .as_ref()
            .and_then(EngineStatus::fault)
            .filter(|_| !self.fault_shown);

        if let Some(fault) = unshown {
            let report = Report::new(fault).to_string();

            eprintln!("audio: {report}");
            self.strip
                .set_status(format!("{RETRY}: audio failed: {report}"));
            self.fault_shown = true;
        }

        unshown.is_some()
    }

    fn change(&mut self, change: &Change, surface: &Surface) {
        let changed = self.strip.apply(change);
        let restarts = change.restarts_audio();
        let retried = !changed && restarts && self.audio_failed();

        if let Some(scale) = change.scale() {
            surface.set_scale(scale);
        }
        if let Change::Bpm(bpm) = *change {
            let tempo = PortIndex(self.host.model().layout().tempo());

            self.host.shared().set(tempo, f32::from(bpm));
            self.strip.show_tempo(f32::from(bpm));
        }

        if (changed && restarts) || retried {
            let retried_device = match change {
                Change::Input(_) if retried => Some(Dir::Input),
                Change::Output(_) if retried => Some(Dir::Output),
                _ => None,
            };

            self.restart(PerDir::from_fn(|dir| retried_device == Some(dir)));
        }
    }

    fn press_strip(&mut self, point: ContentPoint, surface: &Surface) {
        let change = self.strip.press(point);

        // Shows the last listing at once and lists again for the next press.
        if self.strip.lists_devices() {
            self.audio.list();
        }

        if let Some(change) = change {
            self.change(&change, surface);
        }
    }

    fn input(&mut self, event: Event, surface: &Surface) {
        let shared = self.host.shared();
        let write = |index, value| shared.set(index, value);

        match event {
            Event::Press(point) if self.strip.covers(point) => self.press_strip(point, surface),
            Event::Press(point) => {
                self.state
                    .pointer_down(self.host.model(), point.on_screen(), write);
            }
            Event::Move(point) if self.strip.is_dragging() => {
                if let Some(change) = self.strip.drag(point) {
                    self.change(&change, surface);
                }
            }
            Event::Move(point) => {
                self.state
                    .pointer_move(self.host.model(), point.on_screen(), write);
            }
            Event::Release => {
                self.strip.release();
                self.state.pointer_up();
            }
            Event::Wheel(steps, point) if self.strip.covers(point) => {
                if let Some(change) = self.strip.wheel(point, steps) {
                    self.change(&change, surface);
                }
            }
            Event::Wheel(steps, point) => {
                self.state
                    .wheel(self.host.model(), point.on_screen(), steps, write);
            }
            Event::Escape => {
                if !self.strip.close_list() {
                    self.state.escape();
                }
            }
        }
    }

    /// Advances the screen by the time since the last call: whether it changed, and when to look
    /// again without input.
    fn advance(&mut self) -> (bool, Option<Duration>) {
        let now = Instant::now();
        let animating = self
            .state
            .advance((now - self.last).as_secs_f32().min(MAX_STEP));

        self.last = now;

        let plugin_changed = self.take_plugin_changes();
        let replied = self.take_audio_replies();
        let faulted = self.show_fault();

        let interval = if animating || (self.running.is_some() && self.state.shows_outputs()) {
            Some(FRAME_INTERVAL)
        } else {
            self.running.as_ref().map(|_| IDLE_INTERVAL)
        };

        (animating || plugin_changed || replied || faulted, interval)
    }

    /// Shows the window's `scale`, in logical pixels per content pixel.
    fn resized(&mut self, scale: f32) {
        self.strip.show_scale(scale);
    }

    /// The picture at `scale` window pixels per content pixel.
    fn draw(&mut self, scale: f32) -> Option<&Pixmap> {
        let scene = Scene::new(self.host.model(), &self.state, &self.images);

        self.composer.compose(&self.strip, scene, scale)
    }
}

/// The app of the bundle this executable sits in, its audio thread waking the window with `waker`.
fn load(waker: EventLoopProxy<Wake>) -> Result<App, AppError> {
    let bundle = env::current_exe()
        .map_err(|source| AppError::CurrentExe { source })?
        .parent()
        .ok_or(AppError::NoDirectory)?
        .to_owned();
    let path = bundle.join(ui::FILE);
    let json = fs::read(&path).map_err(|source| AppError::ReadModelFile {
        path: path.clone(),
        source,
    })?;
    let model = Model::read(&json).map_err(|source| AppError::ParseModel { path, source })?;
    let library = Library::open(&bundle.join(PLUGIN), model.uri())?;
    let images = render::load_images(&bundle, &model);
    let state = State::new(&model, render::styled_dots(&model, &images));
    let host = Arc::new(Host::new(model, library, bundle));
    let strip = Strip::new(host.model().channels().get() as usize);
    let audio = Audio::spawn(Arc::clone(&host), move || {
        waker.send_event(Wake).ok();
    })
    .map_err(|source| AppError::AudioThread { source })?;

    Ok(App {
        state,
        images,
        strip,
        audio,
        host,
        running: None,
        fault_shown: false,
        composer: Composer::default(),
        last: Instant::now(),
    })
}

fn run() -> Result<(), AppError> {
    let event_loop = EventLoop::with_user_event()
        .build()
        .map_err(|source| AppError::EventLoop { source })?;
    let mut app = load(event_loop.create_proxy())?;
    let title = app.host.model().name().to_owned();

    app.restart(PerDir::default());
    app.audio.list();
    Ok(window::run(event_loop, &title, app)?)
}

fn usage() -> String {
    format!(
        "usage: {PACKAGE} [--help | --version]

Runs the plugin of the bundle this executable sits in on the system's audio devices, under a
preview of its block settings."
    )
}

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);

    match (arguments.next(), arguments.next()) {
        (None, _) => match run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{PACKAGE}: {}", Report::new(error));
                ExitCode::FAILURE
            }
        },
        (Some(flag), None) if flag == "--help" => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
        (Some(flag), None) if flag == "--version" => {
            println!("{PACKAGE} {VERSION}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{}", usage());
            ExitCode::from(2)
        }
    }
}
