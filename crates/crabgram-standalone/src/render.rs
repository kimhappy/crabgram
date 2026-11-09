//! Draws a preview of a block settings page: the plugin's styling images, else built-in widgets.

mod cache;
pub mod canvas;
mod chrome;
mod hit;
pub mod look;
mod renderer;
mod state;
mod stock;
#[cfg(test)]
mod tests;
pub mod text;

pub use renderer::{Images, Renderer, Scene, blit, load_images, resized, screen_size, styled_dots};
pub use state::State;
