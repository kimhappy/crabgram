//! `ui.json` round trips, touch handling, and rendering of the stock look without styling images.

use core::num::NonZeroU32;

use crabgram::{Choice, Controls};
use crabgram_styling::{BlockSettings, ui::Model};
use tiny_skia::Pixmap;

use super::{
    hit::hit,
    look::ScreenPoint,
    renderer::{self as render, Images, Renderer, Scene, origin},
    state::{Overlay, State, Target},
    text::{self, Face},
};
use crate::plugin::PortIndex;

#[derive(Choice)]
enum Wave {
    Sine,
    Square,
}

#[derive(Controls)]
struct Params {
    #[control(default = 3.0, min = 0.1, max = 12.0, unit = Hertz, logarithmic)]
    rate: f32,
    shape: Wave,
    #[control(output)]
    level: f32,
}

fn point((x, y): (f32, f32)) -> ScreenPoint {
    ScreenPoint::new(x, y)
}

fn pressed(state: &mut State, model: &Model, pos: (f32, f32)) -> Vec<(u32, f32)> {
    let mut writes = Vec::new();

    state.pointer_down(model, point(pos), |PortIndex(index), value| {
        writes.push((index, value));
    });
    writes
}

fn wheeled(state: &mut State, model: &Model, pos: (f32, f32), steps: i8) -> Vec<(u32, f32)> {
    let mut writes = Vec::new();

    state.wheel(model, point(pos), steps, |PortIndex(index), value| {
        writes.push((index, value));
    });
    writes
}

fn moved(state: &mut State, model: &Model, pos: (f32, f32)) -> Vec<(u32, f32)> {
    let mut writes = Vec::new();

    state.pointer_move(model, point(pos), |PortIndex(index), value| {
        writes.push((index, value));
    });
    writes
}

fn rendered(model: &Model, state: &State, images: &Images, scale: f32) -> Pixmap {
    render::render(Scene::new(model, state, images), scale)
}

fn model() -> Model {
    model_with(BlockSettings::default())
}

fn model_with(settings: BlockSettings) -> Model {
    Model::new(
        "urn:crabgram:test",
        "Tremolo",
        NonZeroU32::new(2).unwrap(),
        Params::CONTROLS.to_vec(),
        settings,
    )
}

#[test]
fn ui_json_round_trips_every_control() {
    let model = model();
    let mut buffer = Vec::new();

    model
        .write(&mut buffer)
        .expect("writing ui.json never fails on a Vec");
    let read = Model::read(buffer.as_slice()).expect("what write wrote, read reads back");

    assert_eq!(read.name(), model.name());
    assert_eq!(read.layout(), model.layout());
    assert_eq!(read.layout().tempo(), 9);
    assert_eq!(read.controls().len(), model.controls().len());
    assert_eq!(read.controls()[1].labels(), ["Sine", "Square"]);
    assert_eq!(read.controls()[1].symbol(), "shape");
}

#[test]
fn resetting_writes_every_input_back_to_its_default() {
    let model = model();
    let mut state = State::new(&model, None);

    state.set_value(0, 9.0);
    state.set_value(1, 1.0);

    let writes = pressed(&mut state, &model, (1369.0, 5.0)); // "⋮"
    assert_eq!(writes, Vec::new());
    assert_eq!(state.overlay(), Overlay::Menu);

    let writes = pressed(&mut state, &model, (1256.0, 171.0)); // "Reset to default state"
    assert_eq!(
        writes.len(),
        2,
        "the two input controls are reset; the output is not"
    );
    assert_eq!(state.values()[0], Params::RATE.as_ref().default_value());
    assert_eq!(state.values()[1], Params::SHAPE.as_ref().default_value());
}

fn two_pages() -> Model {
    model_with(BlockSettings::default().with_start_padding(6))
}

#[test]
fn a_long_swipe_follows_the_finger_then_slides_to_the_next_page() {
    let model = two_pages();
    let mut state = State::new(&model, None);

    let _ = pressed(&mut state, &model, (700.0, 200.0));
    let _ = moved(&mut state, &model, (700.0 - 356.0, 200.0));
    assert!((state.position() - 0.25).abs() < 1e-3);

    state.pointer_up();
    assert_eq!(state.page(), 1);

    while state.advance(0.016) {}
    assert!((state.position() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_press_on_the_top_bar_never_swipes() {
    let model = two_pages();
    let mut state = State::new(&model, None);

    let _ = pressed(&mut state, &model, (1080.0, 30.0));
    let _ = moved(&mut state, &model, (1080.0 - 356.0, 30.0));
    state.pointer_up();

    assert!(!state.active_scene_only());
    assert_eq!((state.page(), state.position()), (0, 0.0));
}

#[test]
fn a_short_swipe_slides_back_to_its_page() {
    let model = two_pages();
    let mut state = State::new(&model, None);

    let _ = pressed(&mut state, &model, (700.0, 200.0));
    let _ = moved(&mut state, &model, (640.0, 200.0));
    assert!(state.position() > 0.0);

    state.pointer_up();
    assert_eq!(state.page(), 0);

    while state.advance(0.016) {}
    assert!(state.position().abs() < f32::EPSILON);
}

#[test]
fn a_scrolling_screen_draws_between_two_pages() {
    let model = two_pages();
    let mut state = State::new(&model, None);

    state.set_position(0.5);

    let page = rendered(&model, &state, &Images::new(), 1.0);

    assert_eq!((page.width(), page.height()), (1424, 280));
}

#[test]
fn a_renderer_that_remembers_pages_draws_what_a_fresh_one_does() {
    let model = two_pages();
    let images = Images::new();
    let mut renderer = Renderer::default();
    let mut state = State::new(&model, None);

    state.set_position(0.5);
    let _ = renderer.rendered(Scene::new(&model, &state, &images), 2.0);

    state.set_value(0, model.controls()[0].max());

    let remembered = renderer.rendered(Scene::new(&model, &state, &images), 2.0);
    let fresh = rendered(&model, &state, &images, 2.0);

    assert_eq!(remembered.data(), fresh.data());
}

#[test]
fn a_touch_lands_on_a_widget_where_it_is_drawn() {
    let model = model();
    let state = State::new(&model, None);
    let slot = &state.slots()[0];
    let (x, y) = origin(slot);
    let width = crabgram_styling::host::span_width(slot.slots()) as f32;
    let touched = |x| {
        hit(
            state.slots(),
            state.pages(),
            0,
            None,
            ScreenPoint::new(x, y + 1.0),
        )
    };

    assert_eq!(touched(x + 0.5), Some(Target::Widget(slot.position())));
    assert_ne!(
        touched(x + width + 0.5),
        Some(Target::Widget(slot.position()))
    );
}

#[test]
fn a_renderer_draws_a_screen_one_pixel_wider_as_a_fresh_one_does() {
    let (narrow, wide) = (1425.0 / 1424.0, 1426.0 / 1424.0);
    let images = Images::new();
    let mut renderer = Renderer::default();

    for (model, position) in [(model(), 0.0), (two_pages(), 0.5)] {
        let mut state = State::new(&model, None);

        state.set_position(position);
        let _ = renderer.rendered(Scene::new(&model, &state, &images), narrow);

        let remembered = renderer.rendered(Scene::new(&model, &state, &images), wide);
        let fresh = rendered(&model, &state, &images, wide);

        assert_eq!(remembered.data(), fresh.data());
    }
}

#[test]
fn a_moving_output_draws_over_the_remembered_inputs_as_a_fresh_render_does() {
    let model = model();
    let images = Images::new();
    let mut renderer = Renderer::default();
    let mut state = State::new(&model, None);

    let _ = renderer.rendered(Scene::new(&model, &state, &images), 1.0);

    state.set_value(2, 0.75);

    let remembered = renderer.rendered(Scene::new(&model, &state, &images), 1.0);
    let fresh = rendered(&model, &state, &images, 1.0);

    assert_eq!(remembered.data(), fresh.data());
}

#[test]
fn a_press_on_a_list_caption_selects_nothing() {
    let model = model();
    let mut state = State::new(&model, None);

    assert_eq!(pressed(&mut state, &model, (350.0, 108.0)), []);
    assert_eq!(state.values()[1], 0.0);
}

#[test]
fn a_press_on_the_row_below_selects_it() {
    let model = model();
    let mut state = State::new(&model, None);
    let writes = pressed(&mut state, &model, (350.0, 210.0));

    assert_eq!(state.values()[1], 1.0);
    assert_eq!(writes, [(model.layout().control(1), 1.0)]);
}

#[test]
fn a_swipe_puts_back_the_exact_value_a_drag_started_from() {
    let model = model();
    let mut state = State::new(&model, None);

    let _ = pressed(&mut state, &model, (100.0, 170.0));
    let _ = moved(&mut state, &model, (100.0, 120.0));
    assert_ne!(state.values()[0], 3.0);

    let writes = moved(&mut state, &model, (400.0, 120.0));

    assert_eq!(state.values()[0], 3.0);
    assert_eq!(writes, [(model.layout().control(0), 3.0)]);
}

#[test]
fn text_wider_than_its_box_ends_in_an_ellipsis_within_the_box() {
    let fitted = text::fitted(
        Face::Mono,
        15.0,
        "A device name far too long for its box",
        144.0,
    );

    assert!(fitted.ends_with("..."));
    assert!(text::measure(Face::Mono, 15.0, &fitted) <= 144.0);
    assert_eq!(
        text::fitted(Face::Mono, 15.0, "Built-in", 144.0),
        "Built-in"
    );
}

#[test]
fn the_scene_button_switches_between_the_active_scene_and_all() {
    let model = model();
    let mut state = State::new(&model, None);

    assert!(state.active_scene_only());
    let _ = pressed(&mut state, &model, (1080.0, 30.0));
    assert!(!state.active_scene_only());
}

#[test]
fn the_scene_button_answers_on_its_whole_face() {
    let model = model();

    for pos in [(857.0, 10.0), (1186.0, 50.0)] {
        let mut state = State::new(&model, None);
        let _ = pressed(&mut state, &model, pos);
        assert!(!state.active_scene_only(), "{pos:?}");
    }
}

#[test]
fn the_bypass_switch_flips_the_enabled_port_as_last_read() {
    let model = model();
    let mut state = State::new(&model, None);
    let enabled = model.layout().enabled();
    let (x, y) = crabgram_styling::host::position::BYPASS;
    let bypass = (x as f32 + 10.0, y as f32 + 10.0);
    let read = |value| {
        move |PortIndex(index)| {
            (
                if index == enabled { value } else { 0.0 },
                crabgram::ControlState::Normal,
            )
        }
    };

    let _ = state.sync(&model, read(1.0));
    assert_eq!(pressed(&mut state, &model, bypass), [(enabled, 0.0)]);
    assert_eq!(pressed(&mut state, &model, bypass), [(enabled, 0.0)]);

    assert!(state.sync(&model, read(0.0)));
    assert_eq!(pressed(&mut state, &model, bypass), [(enabled, 1.0)]);
}

#[test]
fn the_wheel_fine_tunes_a_knob_by_a_hundredth_of_its_range() {
    let model = model();
    let mut state = State::new(&model, None);
    let rate = Params::RATE.as_ref();
    let start = rate.normalized(state.values()[0]);

    let writes = wheeled(&mut state, &model, (100.0, 170.0), 3);

    assert!((rate.normalized(state.values()[0]) - start - 0.03).abs() < 1e-5);
    assert_eq!(writes, [(model.layout().control(0), state.values()[0])]);

    let _ = wheeled(&mut state, &model, (100.0, 170.0), -3);
    assert!((rate.normalized(state.values()[0]) - start).abs() < 1e-5);
}

#[test]
fn the_wheel_steps_a_list_by_one_label() {
    let model = model();
    let mut state = State::new(&model, None);

    assert_eq!(
        wheeled(&mut state, &model, (350.0, 170.0), -1),
        [(model.layout().control(1), 1.0)]
    );
    let _ = wheeled(&mut state, &model, (350.0, 170.0), -1);
    assert_eq!(state.values()[1], 1.0);
    let _ = wheeled(&mut state, &model, (350.0, 170.0), 1);
    assert_eq!(state.values()[1], 0.0);
}

#[test]
fn the_wheel_off_an_input_widget_changes_nothing() {
    let model = model();
    let mut state = State::new(&model, None);
    let values = state.values().to_vec();
    let (output_x, output_y) = origin(&state.slots()[2]);

    state.set_state(0, crabgram::ControlState::Blocked);

    assert_eq!(state.slots()[2].position(), 2);
    assert_eq!(
        wheeled(&mut state, &model, (output_x + 10.0, output_y + 10.0), 1),
        []
    );
    assert_eq!(wheeled(&mut state, &model, (100.0, 170.0), 1), []);
    assert_eq!(wheeled(&mut state, &model, (100.0, 30.0), 1), []);
    assert_eq!(wheeled(&mut state, &model, (1_000.0, 170.0), 1), []);
    assert_eq!(state.values(), values);
}

#[test]
fn escape_closes_the_menu_or_the_about_page() {
    let model = model();
    let mut state = State::new(&model, None);

    state.set_overlay(Overlay::Menu);
    assert!(state.escape());
    assert_eq!(state.overlay(), Overlay::None);

    state.set_page(1);
    state.set_overlay(Overlay::About);
    assert!(state.escape());
    assert_eq!(state.overlay(), Overlay::None);
    assert_eq!(state.page(), 0);
    assert!(!state.escape());
}

#[test]
fn a_blocked_control_ignores_the_finger() {
    let model = model();
    let mut state = State::new(&model, None);

    state.set_state(0, crabgram::ControlState::Blocked);

    assert_eq!(pressed(&mut state, &model, (100.0, 170.0)), Vec::new());
    assert_eq!(moved(&mut state, &model, (100.0, 120.0)), Vec::new());
}

#[test]
fn a_control_blocked_mid_drag_stops_the_drag_for_good() {
    let model = model();
    let mut state = State::new(&model, None);

    let _ = pressed(&mut state, &model, (100.0, 170.0));
    assert_ne!(moved(&mut state, &model, (100.0, 150.0)), Vec::new());

    let dragged = state.values()[0];

    state.set_state(0, crabgram::ControlState::Blocked);
    assert_eq!(moved(&mut state, &model, (100.0, 120.0)), Vec::new());
    assert_eq!(state.values()[0], dragged);

    state.set_state(0, crabgram::ControlState::Normal);
    assert_eq!(moved(&mut state, &model, (100.0, 100.0)), Vec::new());
    assert_eq!(moved(&mut state, &model, (400.0, 100.0)), Vec::new());
    assert_eq!(state.values()[0], dragged);
}

#[test]
fn a_dimmed_widget_draws_differently_from_a_normal_one() {
    let model = model();
    let mut state = State::new(&model, None);
    let images = Images::new();
    let normal = rendered(&model, &state, &images, 1.0);

    state.set_state(0, crabgram::ControlState::Inactive);

    assert_ne!(rendered(&model, &state, &images, 1.0).data(), normal.data());
}

#[test]
fn widgets_dimmed_one_after_another_draw_as_a_fresh_render_does() {
    let model = model();
    let images = Images::new();
    let mut renderer = Renderer::default();
    let mut state = State::new(&model, None);

    for (scale, dimmed) in [(1.0, 0), (1.13, 1), (1.13, 2), (2.0, 0)] {
        (0..state.states().len()).for_each(|control| {
            state.set_state(control, crabgram::ControlState::Normal);
        });
        state.set_state(dimmed, crabgram::ControlState::Blocked);

        let remembered = renderer.rendered(Scene::new(&model, &state, &images), scale);

        assert_eq!(
            remembered.data(),
            rendered(&model, &state, &images, scale).data(),
            "control {dimmed} at {scale}"
        );
    }
}

#[test]
fn the_back_button_closes_the_about_page() {
    let model = model();
    let mut state = State::new(&model, None);

    state.set_overlay(Overlay::About);
    let _ = pressed(&mut state, &model, (700.0, 200.0));
    assert_eq!(state.overlay(), Overlay::About);

    let _ = pressed(&mut state, &model, (40.0, 30.0)); // back
    assert_eq!(state.overlay(), Overlay::None);
}

#[test]
fn drawn_runs_join_across_short_gaps() {
    let drawn = |index: usize| matches!(index, 2..5 | 20 | 100..102);

    assert_eq!(render::runs(120, drawn), [(2, 21), (100, 102)]);
    assert_eq!(render::runs(10, |_| false), []);
}
