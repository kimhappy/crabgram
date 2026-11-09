//! The window, run by winit and shown by softbuffer.

use core::{mem, num::NonZeroU32};
use std::{rc::Rc, time::Instant};

use tiny_skia::{BlendMode, FilterQuality, Pixmap, PixmapPaint, Transform};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

use crate::{App, CONTENT, render, strip::ContentPoint};

/// Logical pixels per trackpad scroll step.
const PIXELS_PER_STEP: f64 = 32.0;

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Press(ContentPoint),
    /// While the left button is held.
    Move(ContentPoint),
    Release,
    /// Steps scrolled, positive up, with the pointer at a position.
    Wheel(i8, ContentPoint),
    /// Esc pressed.
    Escape,
}

/// Wakes the window from another thread, to take what the thread did.
#[derive(Clone, Copy, Debug)]
pub struct Wake;

pub struct Surface {
    window: Rc<Window>,
    buffer: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    stretched: Option<Pixmap>,
}

/// `picture` stretched to `size` in `target`, which it reuses.
fn stretch<'a>(
    target: &'a mut Option<Pixmap>,
    picture: &Pixmap,
    (width, height): (u32, u32),
) -> Option<&'a Pixmap> {
    let stretched = render::resized(target, (width, height))?;

    stretched.draw_pixmap(
        0,
        0,
        picture.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            blend_mode: BlendMode::Source,
            ..PixmapPaint::default()
        },
        Transform::from_scale(
            width as f32 / picture.width() as f32,
            height as f32 / picture.height() as f32,
        ),
        None,
    );
    Some(stretched)
}

/// Why the window cannot open or be drawn into.
#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    #[error("the window's event loop failed")]
    EventLoop {
        #[source]
        source: winit::error::EventLoopError,
    },
    #[error("cannot open a window")]
    Open {
        #[source]
        source: winit::error::OsError,
    },
    #[error("cannot draw into the window")]
    Draw {
        #[source]
        source: softbuffer::SoftBufferError,
    },
}

impl Surface {
    fn open(event_loop: &ActiveEventLoop, title: &str) -> Result<Self, WindowError> {
        let size = LogicalSize::new(CONTENT.0, CONTENT.1);
        let attributes = Window::default_attributes()
            .with_title(title)
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_max_inner_size(size);
        let window = Rc::new(
            event_loop
                .create_window(attributes)
                .map_err(|source| WindowError::Open { source })?,
        );
        let draw = |source| WindowError::Draw { source };
        let display = softbuffer::Context::new(Rc::clone(&window)).map_err(draw)?;
        let buffer = softbuffer::Surface::new(&display, Rc::clone(&window)).map_err(draw)?;

        Ok(Self {
            window,
            buffer,
            stretched: None,
        })
    }

    /// Resizes the window to `scale` times its content, its size bounds first.
    pub fn set_scale(&self, scale: f32) {
        let (width, height) = CONTENT;
        let size = LogicalSize::new(width as f32 * scale, height as f32 * scale);

        self.window.set_min_inner_size(Some(size));
        self.window.set_max_inner_size(Some(size));
        let _ = self.window.request_inner_size(size);
    }

    fn pixel_ratio(&self) -> f32 {
        self.window.scale_factor() as f32
    }

    /// Logical pixels per content pixel.
    fn scale(&self) -> f32 {
        self.content_scale() / self.pixel_ratio()
    }

    /// Window pixels per content pixel: the scale at which the picture fills the window as the
    /// window manager sized it.
    fn content_scale(&self) -> f32 {
        let size = self.window.inner_size();
        let (content_width, content_height) = CONTENT;
        let scale = (size.width as f32 / content_width as f32)
            .min(size.height as f32 / content_height as f32);

        if scale > 0.0 {
            scale
        } else {
            self.pixel_ratio()
        }
    }

    fn position(&self, at: PhysicalPosition<f64>) -> ContentPoint {
        let size = self.window.inner_size();
        let (content_width, content_height) = CONTENT;

        ContentPoint::new(
            (at.x * f64::from(content_width) / f64::from(size.width.max(1))) as f32,
            (at.y * f64::from(content_height) / f64::from(size.height.max(1))) as f32,
        )
    }

    /// Shows `picture`, stretched if the window's shape differs from the content's; nothing in a
    /// window without area.
    fn show(&mut self, picture: &Pixmap) -> Result<(), WindowError> {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(());
        };
        let fits = (picture.width(), picture.height()) == (size.width, size.height);
        let Some(picture) = (if fits {
            Some(picture)
        } else {
            stretch(&mut self.stretched, picture, (size.width, size.height))
        }) else {
            return Ok(());
        };
        let draw = |source| WindowError::Draw { source };

        self.buffer.resize(width, height).map_err(draw)?;

        let mut buffer = self.buffer.buffer_mut().map_err(draw)?;

        for (pixel, native) in picture.pixels().iter().zip(buffer.iter_mut()) {
            *native = u32::from(pixel.red()) << 16
                | u32::from(pixel.green()) << 8
                | u32::from(pixel.blue());
        }

        buffer.present().map_err(draw)
    }
}

struct Runner<'a> {
    app: App,
    title: &'a str,
    surface: Option<Surface>,
    error: Option<WindowError>,
    cursor: PhysicalPosition<f64>,
    pressed: bool,
    /// Fractional scroll steps not yet passed on.
    wheel: f64,
    /// Input or resize since the last frame.
    touched: bool,
    /// When the moving screen takes its next frame.
    next_frame: Option<Instant>,
}

impl Runner<'_> {
    /// Hides and drops the window at once: stopping the audio afterwards can take a while.
    fn close(&mut self) {
        if let Some(surface) = self.surface.take() {
            surface.window.set_visible(false);
        }
    }

    fn input(&mut self, event: Event) {
        if let Some(surface) = &self.surface {
            self.app.input(event, surface);
            self.touched = true;
        }
    }

    fn scroll(&mut self, delta: MouseScrollDelta) {
        let pixel_ratio = self
            .surface
            .as_ref()
            .map(Surface::pixel_ratio)
            .map_or(1.0, f64::from);

        self.wheel += match delta {
            MouseScrollDelta::LineDelta(_, lines) => f64::from(lines),
            MouseScrollDelta::PixelDelta(pixels) => pixels.y / (PIXELS_PER_STEP * pixel_ratio),
        };

        let steps = self.wheel.trunc();

        self.wheel -= steps;

        if steps != 0.0 {
            self.input(Event::Wheel(
                steps.clamp(-127.0, 127.0) as i8,
                self.position(),
            ));
        }
    }

    fn position(&self) -> ContentPoint {
        self.surface
            .as_ref()
            .map_or(ContentPoint::new(0.0, 0.0), |it| it.position(self.cursor))
    }
}

impl ApplicationHandler<Wake> for Runner<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }

        match Surface::open(event_loop, self.title) {
            Ok(surface) => self.surface = Some(surface),
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.close();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(surface) = &mut self.surface
                    && let Some(picture) = self.app.draw(surface.content_scale())
                    && let Err(error) = surface.show(picture)
                {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(surface) = &self.surface {
                    self.app.resized(surface.scale());
                }
                self.touched = true;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;

                if self.pressed {
                    self.input(Event::Move(self.position()));
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = state == ElementState::Pressed;
                self.input(if self.pressed {
                    Event::Press(self.position())
                } else {
                    Event::Release
                });
            }
            WindowEvent::MouseWheel { delta, .. } => self.scroll(delta),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => self.input(Event::Escape),
            _ => {}
        }
    }

    /// [`Self::about_to_wait`] takes what woke the window.
    fn user_event(&mut self, _: &ActiveEventLoop, _: Wake) {}

    /// Takes a frame on input, or when the moving screen is due one.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(surface) = &self.surface else {
            return;
        };
        let now = Instant::now();

        if !self.touched
            && let Some(next) = self.next_frame.filter(|next| now < *next)
        {
            event_loop.set_control_flow(ControlFlow::WaitUntil(next));
            return;
        }

        let (changed, interval) = self.app.advance();

        if mem::take(&mut self.touched) || changed {
            surface.window.request_redraw();
        }

        self.next_frame = interval.map(|interval| now + interval);
        event_loop.set_control_flow(
            self.next_frame
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
        );
    }
}

/// Opens a `title`d window of [`CONTENT`] screen pixels and runs `app` on `event_loop` until it
/// closes.
///
/// # Errors
///
/// When the window cannot be opened or drawn into.
pub fn run(event_loop: EventLoop<Wake>, title: &str, app: App) -> Result<(), WindowError> {
    let mut runner = Runner {
        app,
        title,
        surface: None,
        error: None,
        cursor: PhysicalPosition::new(0.0, 0.0),
        pressed: false,
        wheel: 0.0,
        touched: true,
        next_frame: None,
    };

    let ran = event_loop
        .run_app(&mut runner)
        .map_err(|source| WindowError::EventLoop { source });

    runner.close();
    drop(runner.app);
    ran?;
    runner.error.map_or(Ok(()), Err)
}
