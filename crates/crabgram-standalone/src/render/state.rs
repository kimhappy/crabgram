//! What the preview shows, and how a touch changes a port.

use crabgram::{Control, ControlState, Kind};
use crabgram_styling::{Slot, host, ui::Model};

use super::look::{self, Area, MenuAction, ScreenPoint};
use crate::plugin::PortIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Bypass,
    /// Switches between controlling the active scene and all scenes.
    Scenes,
    More,
    Dot(u32),
    Widget(usize),
}

/// What the focus ring is drawn around once touched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Bypass,
    /// The widget of the control at this index.
    Widget(usize),
}

/// A drag of a knob or fader: the control, where it started and its value then.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ValueDrag {
    control: usize,
    start_y: f32,
    start_value: f32,
}

/// What covers the pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    /// The "⋮" menu.
    Menu,
    About,
}

#[derive(Clone, Debug)]
pub struct State {
    values: Vec<f32>,
    /// The state the plugin put each control in.
    states: Vec<ControlState>,
    /// `lv2:enabled` inverted, as last read.
    bypassed: bool,
    /// The bypass switch or the widget last touched.
    focus: Option<Focus>,
    /// Whether the scene button shows "Control Active" rather than "Control All".
    active_scene_only: bool,
    /// The page the screen scrolls to, chosen by a dot or a swipe.
    page: u32,
    /// Where the screen is, in pages; fractional while scrolling or sliding to [`Self::page`].
    position: f32,
    overlay: Overlay,
    /// Scrolls the menu's long labels.
    menu_seconds: f32,
    slots: Vec<Slot>,
    /// At least 1.
    pages: u32,
    /// Where the styled pagination dots sit; none for the stock dots.
    dots: Option<Area>,
    drag: Option<ValueDrag>,
    swipe: Option<Swipe>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Swipe {
    from: ScreenPoint,
    start_position: f32,
    active: bool,
    moved: f32,
}

/// Sideways travel in screen pixels before the pages follow the finger.
const SWIPE_START: f32 = 16.0;

/// Swipe travel in screen pixels needed to leave the starting page.
const SWIPE_DISTANCE: f32 = 120.0;

/// Exponential slide rate per second.
const SLIDE_RATE: f32 = 14.0;

/// Vertical drag in screen pixels that turns a knob through its whole range.
const PIXELS_PER_RANGE: f32 = 150.0;

/// Share of a knob's or fader's range one wheel step turns it by.
const WHEEL_STEP: f32 = 0.01;

impl State {
    /// Takes touches on the styled pagination `dots`, one equal column per page.
    #[must_use]
    pub fn new(model: &Model, dots: Option<Area>) -> Self {
        Self {
            values: model
                .controls()
                .iter()
                .map(Control::default_value)
                .collect(),
            states: vec![ControlState::Normal; model.controls().len()],
            bypassed: false,
            focus: None,
            active_scene_only: true,
            page: 0,
            position: 0.0,
            overlay: Overlay::None,
            menu_seconds: 0.0,
            slots: model.settings().slots(model.controls()),
            pages: model.settings().pages(model.controls()),
            dots,
            drag: None,
            swipe: None,
        }
    }

    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    #[must_use]
    pub fn states(&self) -> &[ControlState] {
        &self.states
    }

    #[must_use]
    pub const fn bypassed(&self) -> bool {
        self.bypassed
    }

    #[must_use]
    pub const fn focus(&self) -> Option<Focus> {
        self.focus
    }

    #[must_use]
    pub const fn active_scene_only(&self) -> bool {
        self.active_scene_only
    }

    #[must_use]
    pub const fn position(&self) -> f32 {
        self.position
    }

    #[must_use]
    pub const fn overlay(&self) -> Overlay {
        self.overlay
    }

    #[must_use]
    pub const fn menu_seconds(&self) -> f32 {
        self.menu_seconds
    }

    #[must_use]
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    #[must_use]
    pub const fn pages(&self) -> u32 {
        self.pages
    }

    #[must_use]
    pub const fn dots(&self) -> Option<Area> {
        self.dots
    }

    /// Takes the bypass and each control's value and state from the plugin, as `port` reads them;
    /// whether the screen shows a change.
    pub fn sync(&mut self, model: &Model, port: impl Fn(PortIndex) -> (f32, ControlState)) -> bool {
        let shows_outputs = self.shows_outputs();
        let layout = model.layout();
        let bypassed = port(PortIndex(layout.enabled())).0 <= 0.0;
        let bypass_changed = core::mem::replace(&mut self.bypassed, bypassed) != bypassed;

        model
            .controls()
            .iter()
            .enumerate()
            .fold(bypass_changed, |changed, (position, control)| {
                let (value, state) = port(PortIndex(layout.control(position as u32)));
                let value = control.clamp(value);
                let moved = self.values[position].to_bits() != value.to_bits();
                let restated = self.states[position] != state;

                self.values[position] = value;
                self.states[position] = state;
                changed || restated || (moved && (shows_outputs || !control.is_output()))
            })
    }

    /// Handles a press at `point`, calling `write` with the index and value of every port it
    /// sets.
    pub fn pointer_down(
        &mut self,
        model: &Model,
        point: ScreenPoint,
        mut write: impl FnMut(PortIndex, f32),
    ) {
        match self.overlay {
            Overlay::About => {
                if super::hit::on_close(point) {
                    self.close_about();
                }
                return;
            }
            Overlay::Menu => {
                self.overlay = Overlay::None;

                match super::hit::hit_menu(model, point) {
                    Some(MenuAction::Reset) => self.reset(model, &mut write),
                    Some(MenuAction::About) => self.overlay = Overlay::About,
                    Some(MenuAction::Inert) | None => {}
                }
                return;
            }
            Overlay::None => {}
        }

        let shown = self.position.round() as u32;
        let target = super::hit::hit(&self.slots, self.pages, shown, self.dots, point);

        if matches!(target, None | Some(Target::Widget(_))) {
            self.swipe = Some(Swipe {
                from: point,
                start_position: self.position,
                active: false,
                moved: 0.0,
            });
        }

        let Some(target) = target else {
            return;
        };

        match target {
            Target::Scenes => self.active_scene_only = !self.active_scene_only,
            Target::Bypass => {
                self.focus = Some(Focus::Bypass);
                write(
                    PortIndex(model.layout().enabled()),
                    f32::from(self.bypassed),
                );
            }
            Target::More => {
                self.overlay = Overlay::Menu;
                self.menu_seconds = 0.0;
            }
            Target::Dot(page) => self.page = page,
            Target::Widget(control) => {
                self.focus = Some(Focus::Widget(control));
                self.press_widget(model, control, point, &mut write);
            }
        }
    }

    /// Back to the first page, as the about page's back button does.
    const fn close_about(&mut self) {
        self.overlay = Overlay::None;
        self.page = 0;
        self.position = 0.0;
    }

    /// Closes the menu or the about page, as Esc does; whether one was open.
    pub const fn escape(&mut self) -> bool {
        match self.overlay {
            Overlay::None => return false,
            Overlay::Menu => self.overlay = Overlay::None,
            Overlay::About => self.close_about(),
        }
        true
    }

    fn press_widget(
        &mut self,
        model: &Model,
        control: usize,
        point: ScreenPoint,
        write: &mut impl FnMut(PortIndex, f32),
    ) {
        let info = model.controls()[control];

        if info.is_output() || self.refuses_touch(control) {
            return;
        }

        match info.kind() {
            Kind::Toggle { .. } => {
                self.values[control] = f32::from(self.values[control] <= 0.0);
                self.write(model, control, write);
            }
            Kind::Choice { .. } => {
                let offset = ((point.y - host::WIDGET_TOP as f32 - look::ROW_TOP)
                    / look::ROW_HEIGHT)
                    .floor();

                if look::row_fits(offset) {
                    self.values[control] =
                        info.quantize(info.label_index(self.values[control]) as f32 + offset);
                    self.write(model, control, write);
                }
            }
            Kind::Float { .. } | Kind::Integer { .. } => {
                let value = self.values[control];

                self.drag = Some(ValueDrag {
                    control,
                    start_y: point.y,
                    start_value: value,
                });
            }
        }
    }

    /// Whether the plugin blocked `control`.
    fn refuses_touch(&self, control: usize) -> bool {
        self.states[control] == ControlState::Blocked
    }

    /// Handles the pointer moving while pressed: drags a knob or fader or swipes pages, calling
    /// `write` with the index and value of every port it sets. A drag ends for good once the
    /// plugin blocks its control.
    pub fn pointer_move(
        &mut self,
        model: &Model,
        point: ScreenPoint,
        mut write: impl FnMut(PortIndex, f32),
    ) {
        self.drag = self.drag.filter(|drag| !self.refuses_touch(drag.control));

        if self.swiped(model, point, &mut write) {
            return;
        }

        let Some(drag) = self.drag else {
            return;
        };
        let info = model.controls()[drag.control];
        let normalized =
            info.normalized(drag.start_value) + (drag.start_y - point.y) / PIXELS_PER_RANGE;

        self.values[drag.control] = info.quantize(info.denormalized(normalized));
        self.write(model, drag.control, &mut write);
    }

    /// Turns the widget under `point` by `steps` wheel steps, positive up, calling `write` with the
    /// index and value of the port it sets. A step is a hundredth of a knob or fader, one of an
    /// integer and one label of a list.
    pub fn wheel(
        &mut self,
        model: &Model,
        point: ScreenPoint,
        steps: i8,
        mut write: impl FnMut(PortIndex, f32),
    ) {
        if self.overlay != Overlay::None {
            return;
        }

        let shown = self.position.round() as u32;
        let Some(Target::Widget(control)) =
            super::hit::hit(&self.slots, self.pages, shown, self.dots, point)
        else {
            return;
        };
        let info = model.controls()[control];

        if info.is_output() || self.refuses_touch(control) {
            return;
        }

        let (value, steps) = (self.values[control], f32::from(steps));

        self.values[control] = match info.kind() {
            Kind::Float { .. } => {
                info.denormalized(steps.mul_add(WHEEL_STEP, info.normalized(value)))
            }
            Kind::Integer { .. } => info.quantize(value + steps),
            Kind::Choice { .. } => info.quantize(value - steps),
            Kind::Toggle { .. } => return,
        };
        self.focus = Some(Focus::Widget(control));
        self.write(model, control, &mut write);
    }

    /// Ends any drag and lets a swipe settle on a page.
    pub fn pointer_up(&mut self) {
        self.drag = None;

        if let Some(swipe) = self.swipe.take().filter(|swipe| swipe.active) {
            let pages = self.pages;
            let moved = swipe.moved;
            let start = swipe.start_position.round() as u32;

            self.page = if moved <= -SWIPE_DISTANCE {
                (start + 1).min(pages - 1)
            } else if moved >= SWIPE_DISTANCE {
                start.saturating_sub(1)
            } else {
                start
            };
        }
    }

    /// Whether an output widget is on screen, where the plugin can move it.
    #[must_use]
    pub fn shows_outputs(&self) -> bool {
        let pages = self.position.floor() as u32..=self.position.ceil() as u32;

        self.overlay != Overlay::About
            && self
                .slots
                .iter()
                .any(|slot| slot.control().is_output() && pages.contains(&slot.page()))
    }

    /// Slides toward [`Self::page`] unless a finger holds the pages. Whether the screen changed.
    pub fn advance(&mut self, seconds: f32) -> bool {
        if self.overlay == Overlay::Menu {
            self.menu_seconds += seconds;
            return true;
        }

        let target = self.page as f32;

        #[expect(clippy::float_cmp, reason = "the slide ends on the page exactly")]
        let settled = self.position == target;

        if self.swipe.is_some_and(|swipe| swipe.active) || settled {
            return false;
        }

        let remaining = target - self.position;

        self.position = if remaining.abs() < 0.002 {
            target
        } else {
            self.position + remaining * (1.0 - (-SLIDE_RATE * seconds).exp())
        };
        true
    }

    /// Makes the pages follow the pointer once it moved sideways, restoring the value of a
    /// drag the press started. Whether the movement is a swipe.
    fn swiped(
        &mut self,
        model: &Model,
        point: ScreenPoint,
        write: &mut impl FnMut(PortIndex, f32),
    ) -> bool {
        let Some(swipe) = self.swipe.as_mut() else {
            return false;
        };
        let (moved_x, moved_y) = (point.x - swipe.from.x, point.y - swipe.from.y);

        if !swipe.active && (moved_x.abs() < SWIPE_START || moved_x.abs() < 2.0 * moved_y.abs()) {
            return false;
        }

        let started = !swipe.active;
        let (screen_width, _) = host::SCREEN;
        let pages = self.pages;

        swipe.active = true;
        swipe.moved = moved_x;
        self.position =
            (swipe.start_position - moved_x / screen_width as f32).clamp(0.0, (pages - 1) as f32);

        if let Some(drag) = self.drag.take_if(|_| started) {
            self.values[drag.control] = drag.start_value;
            self.write(model, drag.control, write);
        }

        true
    }

    fn write(&self, model: &Model, control: usize, write: &mut impl FnMut(PortIndex, f32)) {
        write(
            PortIndex(model.layout().control(control as u32)),
            self.values[control],
        );
    }

    /// "Reset to default state" of the "⋮" menu.
    fn reset(&mut self, model: &Model, write: &mut impl FnMut(PortIndex, f32)) {
        for (index, control) in model
            .controls()
            .iter()
            .enumerate()
            .filter(|(_, control)| !control.is_output())
        {
            self.values[index] = control.default_value();
            self.write(model, index, write);
        }
    }

    #[cfg(test)]
    pub const fn page(&self) -> u32 {
        self.page
    }

    #[cfg(test)]
    pub fn set_value(&mut self, control: usize, value: f32) {
        self.values[control] = value;
    }

    #[cfg(test)]
    pub fn set_state(&mut self, control: usize, state: ControlState) {
        self.states[control] = state;
    }

    #[cfg(test)]
    pub const fn set_position(&mut self, position: f32) {
        self.position = position;
    }

    #[cfg(test)]
    pub const fn set_page(&mut self, page: u32) {
        self.page = page;
    }

    #[cfg(test)]
    pub const fn set_overlay(&mut self, overlay: Overlay) {
        self.overlay = overlay;
    }
}
