use core::ops::Range;

use crabgram::Control;

use crate::{BlockSettings, Parameter, host, model::getters};

/// Where one control's widget sits on the block settings screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    control: Control,
    position: usize,
    first: u32,
    slots: u32,
}

impl Slot {
    getters! {
        /// The placed control.
        const control -> Control = |it| it.control;

        /// Index among the controls, in port order.
        const position -> usize = |it| it.position;

        /// First slot, counted from 0 across all pages.
        const first -> u32 = |it| it.first;

        /// Number of slots taken.
        const slots -> u32 = |it| it.slots;
    }

    /// Page of the first slot, from 0.
    #[must_use]
    pub const fn page(&self) -> u32 {
        self.first / host::SLOTS_PER_PAGE
    }

    /// Column of the first slot on its page (0 to 5).
    #[must_use]
    pub const fn column(&self) -> u32 {
        self.first % host::SLOTS_PER_PAGE
    }

    /// Left edge of the widget on its page.
    #[must_use]
    pub const fn x(&self) -> u32 {
        host::slot_x(self.column())
    }

    pub(crate) const fn range(&self) -> Range<u32> {
        self.first..self.first.saturating_add(self.slots)
    }

    /// Page of the last slot, from 0.
    pub(crate) const fn last_page(&self) -> u32 {
        (self.range().end - 1) / host::SLOTS_PER_PAGE
    }

    /// Whether the two share a slot.
    pub(crate) const fn overlaps(&self, other: &Self) -> bool {
        overlap(&self.range(), &other.range())
    }
}

fn first_free(first: u32, slots: u32, taken: impl Fn(&Range<u32>) -> bool) -> u32 {
    (first..=u32::MAX)
        .find(|start| !taken(&(*start..start.saturating_add(slots))))
        .unwrap_or(first)
}

const fn overlap(one: &Range<u32>, other: &Range<u32>) -> bool {
    one.start < other.end && other.start < one.end
}

impl BlockSettings {
    /// Lays out `controls` (in port order): after the padding, around pinned
    /// widgets ([`Parameter::pin`](crate::Parameter::pin)); hidden controls get no slot.
    ///
    /// A multi-slot list may cross a page edge, which [`Problem::CrossesPage`](crate::Problem::CrossesPage) reports.
    ///
    /// ```
    /// use crabgram::{Choice, Controls};
    /// use crabgram_styling::{BlockSettings, HostStyle, Parameter, Slot};
    ///
    /// #[derive(Choice)]
    /// enum Wave {
    ///     Sine,
    ///     Square,
    /// }
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     rate: f32,
    ///     wave: Wave,
    ///     #[control(output)]
    ///     level: f32,
    /// }
    ///
    /// let settings = BlockSettings::default()
    ///     .with_start_padding(1)
    ///     .with_parameter(Parameter::of(Params::WAVE).with_style(HostStyle::ListWith2Slots))
    ///     .with_parameter(Parameter::of(Params::LEVEL).with_pin(0));
    ///
    /// let firsts: Vec<u32> = settings
    ///     .slots(Params::CONTROLS)
    ///     .iter()
    ///     .map(Slot::first)
    ///     .collect();
    ///
    /// assert_eq!(firsts, [1, 2, 0]);
    /// ```
    #[must_use]
    pub fn slots(&self, controls: &[Control]) -> Vec<Slot> {
        let shown = || {
            controls
                .iter()
                .enumerate()
                .filter(|(_, it)| !it.is_hidden())
        };
        let pinned: Vec<Range<u32>> = shown()
            .filter_map(|(_, control)| {
                let parameter = self.parameter_of(control)?;
                parameter
                    .pin
                    .map(|at| at..at.saturating_add(parameter.slots()))
            })
            .collect();
        let taken = |slots: &Range<u32>| pinned.iter().any(|it| overlap(it, slots));

        shown()
            .scan(self.start_padding, |next, (position, control)| {
                let parameter = self.parameter_of(control);
                let slots = parameter.map_or(1, Parameter::slots);
                let first = parameter.and_then(|it| it.pin).unwrap_or_else(|| {
                    let free = first_free(*next, slots, taken);

                    *next = free.saturating_add(slots);
                    free
                });

                Some(Slot {
                    control: *control,
                    position,
                    first,
                    slots,
                })
            })
            .collect()
    }

    /// Pages needed for the last used slot; at least 1.
    #[must_use]
    pub fn pages(&self, controls: &[Control]) -> u32 {
        self.slots(controls)
            .iter()
            .map(|slot| slot.range().end)
            .max()
            .unwrap_or(0)
            .div_ceil(host::SLOTS_PER_PAGE)
            .max(1)
    }
}

#[cfg(test)]
mod tests {
    use crabgram::{Choice, Controls};

    use crate::{BlockSettings, HostStyle, Parameter, Widget};

    #[derive(Debug, PartialEq, Choice)]
    enum Wave {
        Sine,
        Square,
    }

    #[derive(Controls)]
    struct Params {
        wave: Wave,
        rate: f32,
    }

    #[test]
    fn a_list_given_images_after_a_style_takes_one_slot() {
        let settings = BlockSettings::default().with_parameter(
            Parameter::of(Params::WAVE)
                .with_style(HostStyle::ListWith3Slots)
                .with_widget(Widget::new("asset/wave.png")),
        );

        let placed: Vec<(u32, u32)> = settings
            .slots(Params::CONTROLS)
            .iter()
            .map(|slot| (slot.first(), slot.slots()))
            .collect();

        assert_eq!(placed, [(0, 1), (1, 1)]);
    }

    #[test]
    fn a_pinned_parameter_takes_the_default_background() {
        let knob = || Widget::new("asset/knob.png").with_background("asset/panel.png");
        let rate = &Params::CONTROLS[1];
        let pinned = BlockSettings::default()
            .with_knob(knob())
            .with_parameter(Parameter::of(Params::RATE).with_pin(3));
        let unpinned = pinned.clone().with_parameter(Parameter::of(Params::RATE));

        assert_eq!(pinned.widget_of(rate), None);
        assert_eq!(
            pinned.stock_background(rate).map(|it| it.path.as_str()),
            Some("asset/panel.png")
        );
        assert_eq!(unpinned.widget_of(rate), Some(&knob()));
    }

    #[test]
    fn a_styled_list_takes_the_default_list_background() {
        let settings = BlockSettings::default()
            .with_list(Widget::new("asset/list-frames.png").with_background("asset/list.png"))
            .with_parameter(
                Parameter::of(Params::CONTROLS[0]).with_style(HostStyle::ListWith2Slots),
            );

        assert_eq!(
            settings
                .stock_background(Params::CONTROLS[0])
                .map(|it| it.path.as_str()),
            Some("asset/list.png")
        );
    }
}
