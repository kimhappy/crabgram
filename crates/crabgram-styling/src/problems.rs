use std::collections::HashSet;

use crabgram::{Control, WidgetKind};

use crate::{
    Animation, BlockSettings, HostStyle, Parameter, Slot, Styling, Widget, host,
    model::WidgetSource,
};

/// A mistake in a [`Styling`], from [`Styling::problems`].
#[derive(Clone, Debug, Eq, PartialEq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum Problem {
    /// The block image draws a sprite for a symbol that is no control.
    #[error("the block image draws {symbol}, which is no control")]
    BlockImageOfNoControl {
        /// The symbol the sprite names.
        symbol: String,
    },
    /// An image path the bundle cannot ship or Turtle cannot write.
    #[error("{path:?} {reason}")]
    PathInvalid {
        /// The image path.
        path: String,
        /// What is wrong with it.
        reason: PathReason,
    },
    /// The start padding exceeds [`host::MAX_START_PADDING`].
    #[error(
        "a start padding of {padding} exceeds the limit of {}",
        host::MAX_START_PADDING
    )]
    StartPaddingPastLimit {
        /// The padding asked for.
        padding: u32,
    },
    /// A styled symbol is no control.
    #[error("{symbol} is styled, but it is no control")]
    StyledNoControl {
        /// The symbol.
        symbol: String,
    },
    /// A styled control is hidden.
    #[error("{symbol} is styled, but it is hidden")]
    StyledHidden {
        /// The control symbol.
        symbol: String,
    },
    /// Two widgets share a slot.
    #[error("{symbol} overlaps {other} in slot {slot}, counted from 0 like a pin")]
    Overlap {
        /// The later control.
        symbol: String,
        /// The earlier control it overlaps.
        other: String,
        /// The first slot they share, from 0 like [`Parameter::pin`].
        slot: u32,
    },
    /// A widget has a scenes background, but its control never [follows
    /// scenes](Control::follows_scenes).
    #[error("{symbol} has a scenes background, but scenes leave it alone")]
    BackgroundScenesNeverShown {
        /// The control symbol.
        symbol: String,
    },
    /// A widget has an overlay its control never shows: `dgcs:blocked` on a control that is not
    /// [blockable](Control::is_blockable), or any overlay on an output.
    #[error("{symbol} never shows its {overlay} overlay")]
    OverlayNeverShown {
        /// The control symbol.
        symbol: String,
        /// The overlay predicate, `dgcs:blocked` or `dgcs:inactive`.
        overlay: &'static str,
    },
    /// A widget crosses from one page to the next.
    #[error("{symbol} crosses from page {page} to the next")]
    CrossesPage {
        /// The control symbol.
        symbol: String,
        /// The page it starts on, from 1.
        page: u32,
    },
    /// A host style on a control whose widget it does not fit.
    #[error(
        "{symbol} uses {}, which only fits a {}, but it is a {}",
        style.curie(),
        style.kind().name(),
        widget.name()
    )]
    StyleOnWrongWidget {
        /// The control symbol.
        symbol: String,
        /// The host style.
        style: HostStyle,
        /// The widget the control has.
        widget: WidgetKind,
    },
}

/// Why an image path is invalid, the first that applies.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum PathReason {
    /// The path is absolute, has a scheme or climbs out with `..`.
    #[error("leaves the bundle")]
    LeavesBundle,
    /// The path holds an empty or `.` component.
    #[error("holds an empty or `.` component: name the file plainly, as asset/<file>")]
    NotPlain,
    /// The path holds a character Turtle cannot write in an IRI.
    #[error("holds a character Turtle forbids in an IRI")]
    NotIri,
    /// The path holds `%`.
    #[error("holds %, which an image path must avoid")]
    Escaped,
}

impl PathReason {
    /// The first reason `path` is invalid, or `None` for a plain relative path inside the bundle.
    ///
    /// ```
    /// use crabgram_styling::PathReason;
    ///
    /// assert_eq!(PathReason::of("asset/knob.png"), None);
    /// assert_eq!(PathReason::of("asset/knob:1.png"), None);
    /// assert_eq!(
    ///     PathReason::of("../knob.png"),
    ///     Some(PathReason::LeavesBundle)
    /// );
    /// assert_eq!(
    ///     PathReason::of("file:knob.png"),
    ///     Some(PathReason::LeavesBundle)
    /// );
    /// ```
    #[must_use]
    pub fn of(path: &str) -> Option<Self> {
        let component = |matches: fn(&str) -> bool| path.split('/').any(matches);
        let scheme = path.split('/').next().is_some_and(|it| it.contains(':'));

        if path.starts_with('/') || scheme || component(|it| it == "..") {
            Some(Self::LeavesBundle)
        } else if component(|it| matches!(it, "" | ".")) {
            Some(Self::NotPlain)
        } else if !crabgram::__private::fits_iri(path.as_bytes()) {
            Some(Self::NotIri)
        } else if path.contains('%') {
            Some(Self::Escaped)
        } else {
            None
        }
    }
}

impl Styling {
    /// Every [`Problem`] in the styling, for the controls given.
    ///
    /// ```
    /// use crabgram::{Controls, WidgetKind};
    /// use crabgram_styling::{BlockSettings, HostStyle, Parameter, Problem, Styling};
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     rate: f32,
    /// }
    ///
    /// let styling = Styling::default().with_block_settings(
    ///     BlockSettings::default()
    ///         .with_parameter(Parameter::of(Params::RATE).with_style(HostStyle::ListWith2Slots)),
    /// );
    /// let problems = styling.problems(Params::CONTROLS);
    ///
    /// assert_eq!(
    ///     problems,
    ///     [Problem::StyleOnWrongWidget {
    ///         symbol: "rate".to_owned(),
    ///         style: HostStyle::ListWith2Slots,
    ///         widget: WidgetKind::Knob,
    ///     }]
    /// );
    /// ```
    #[must_use]
    pub fn problems(&self, controls: &[Control]) -> Vec<Problem> {
        let block = self
            .block_image
            .iter()
            .flat_map(|block| &block.animations)
            .filter(|Animation { symbol, .. }| controls.iter().all(|it| it.symbol() != symbol))
            .map(|Animation { symbol, .. }| Problem::BlockImageOfNoControl {
                symbol: symbol.clone(),
            });
        let settings = self
            .block_settings
            .iter()
            .flat_map(|settings| settings_problems(settings, controls));
        let invalid_paths = self
            .framings(controls)
            .into_iter()
            .filter_map(|(path, _)| path_problem(path));
        let mut seen = HashSet::new();

        block
            .chain(settings)
            .chain(invalid_paths)
            .filter(|problem| seen.insert(problem.clone()))
            .collect()
    }
}

fn path_problem(path: &str) -> Option<Problem> {
    PathReason::of(path).map(|reason| Problem::PathInvalid {
        path: path.to_owned(),
        reason,
    })
}

fn settings_problems(settings: &BlockSettings, controls: &[Control]) -> Vec<Problem> {
    let padding = (settings.start_padding > host::MAX_START_PADDING).then_some(
        Problem::StartPaddingPastLimit {
            padding: settings.start_padding,
        },
    );
    let parameters = settings
        .parameters
        .iter()
        .flat_map(|parameter| parameter_problems(parameter, controls));
    let widgets = controls
        .iter()
        .filter(|control| !control.is_hidden())
        .filter_map(|control| Some((control, own_widget(settings, control)?)))
        .flat_map(|(control, widget)| widget_problems(control, widget));
    let slots = settings.slots(controls);
    let layout = slots
        .iter()
        .enumerate()
        .flat_map(|(position, slot)| slot_problems(&slots[..position], slot));

    padding
        .into_iter()
        .chain(parameters)
        .chain(widgets)
        .chain(layout)
        .collect()
}

/// The widget `control` shows, when every control showing it is alike: its parameter's own, or
/// the default meter, which only outputs show.
fn own_widget<'a>(settings: &'a BlockSettings, control: &Control) -> Option<&'a Widget> {
    settings
        .sourced_widget_of(control)
        .filter(|(source, _)| {
            matches!(
                source,
                WidgetSource::Parameter(_) | WidgetSource::Default(WidgetKind::Meter)
            )
        })
        .map(|(_, widget)| widget)
}

fn widget_problems<'a>(
    control: &'a Control,
    widget: &'a Widget,
) -> impl Iterator<Item = Problem> + use<'a> {
    let symbol = move || control.symbol().to_owned();
    let scenes = (widget.background_scenes.is_some() && !control.follows_scenes())
        .then(|| Problem::BackgroundScenesNeverShown { symbol: symbol() });
    let overlays = widget
        .overlays
        .paths()
        .filter(|(overlay, _)| {
            control.is_output() || (*overlay == "dgcs:blocked" && !control.is_blockable())
        })
        .map(move |(overlay, _)| Problem::OverlayNeverShown {
            symbol: symbol(),
            overlay,
        });

    scenes.into_iter().chain(overlays)
}

fn parameter_problems(
    parameter: &Parameter,
    controls: &[Control],
) -> impl Iterator<Item = Problem> + use<> {
    let symbol = || parameter.symbol.clone();
    let control = controls.iter().find(|it| it.symbol() == parameter.symbol);
    let unknown = control
        .is_none()
        .then(|| Problem::StyledNoControl { symbol: symbol() });
    let hidden = control
        .filter(|it| it.is_hidden())
        .map(|_| Problem::StyledHidden { symbol: symbol() });
    let mismatch = control
        .zip(parameter.style())
        .filter(|(control, style)| control.widget() != style.kind())
        .map(|(control, style)| Problem::StyleOnWrongWidget {
            symbol: symbol(),
            style,
            widget: control.widget(),
        });

    unknown.into_iter().chain(hidden).chain(mismatch)
}

fn slot_problems(earlier: &[Slot], slot: &Slot) -> impl Iterator<Item = Problem> {
    let symbol = || slot.control().symbol().to_owned();
    let overlap = earlier
        .iter()
        .find(|other| other.overlaps(slot))
        .map(|other| Problem::Overlap {
            symbol: symbol(),
            other: other.control().symbol().to_owned(),
            slot: other.first().max(slot.first()),
        });
    let crossing = (slot.page() != slot.last_page()).then(|| Problem::CrossesPage {
        symbol: symbol(),
        page: slot.page() + 1,
    });

    overlap.into_iter().chain(crossing)
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use crabgram::{Control, Controls};

    use super::PathReason;
    use crate::{
        BlockImage, BlockSettings, Frame, HostStyle, Icons, Overlays, Pagination, Parameter,
        Sprite, Styling, Widget,
    };

    fn messages(styling: &Styling, controls: &[Control]) -> Vec<String> {
        styling
            .problems(controls)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[derive(Controls)]
    struct Params {
        level: f32,
    }

    #[derive(Controls)]
    struct Other {
        gain: f32,
        #[control(output)]
        lfo: f32,
    }

    const LEVEL: Control = Params::CONTROLS[0];

    fn settings(settings: BlockSettings) -> Styling {
        Styling {
            block_settings: Some(settings),
            ..Styling::default()
        }
    }

    #[test]
    fn a_path_turtle_cannot_write_is_a_problem_once() {
        let styling = Styling {
            icons: Some(Icons {
                off: "asset/block off.png".into(),
                on: "asset/block-on.png".into(),
            }),
            block_image: None,
            block_settings: Some(
                BlockSettings::default()
                    .with_knob(Widget::new("asset/<knob>.png"))
                    .with_meter(Widget::new("asset/<knob>.png")),
            ),
        };

        assert_eq!(
            messages(&styling, &[LEVEL]),
            [
                r#""asset/block off.png" holds a character Turtle forbids in an IRI"#,
                r#""asset/<knob>.png" holds a character Turtle forbids in an IRI"#,
            ]
        );
    }

    #[test]
    fn a_path_outside_the_bundle_is_a_problem() {
        let styling = settings(
            BlockSettings::default()
                .with_background("../background.png")
                .with_about("/root/about.png")
                .with_pagination(Pagination::Dots("asset/../../dots.png".into())),
        );

        assert_eq!(
            messages(&styling, &[LEVEL]),
            [
                r#""../background.png" leaves the bundle"#,
                r#""/root/about.png" leaves the bundle"#,
                r#""asset/../../dots.png" leaves the bundle"#,
            ]
        );
    }

    #[test]
    fn a_path_with_a_dot_or_an_empty_component_is_a_problem() {
        let styling = settings(
            BlockSettings::default()
                .with_background("asset/./background.png")
                .with_about("asset//about.png"),
        );

        assert_eq!(
            messages(&styling, &[LEVEL]),
            [
                r#""asset/./background.png" holds an empty or `.` component: name the file plainly, as asset/<file>"#,
                r#""asset//about.png" holds an empty or `.` component: name the file plainly, as asset/<file>"#,
            ]
        );
    }

    #[test]
    fn a_path_with_a_scheme_is_a_problem_but_one_with_a_hash_or_a_question_mark_is_not() {
        let styling = settings(
            BlockSettings::default()
                .with_background("file:///etc/background.png")
                .with_about("asset/about#2?.png"),
        );

        assert_eq!(
            messages(&styling, &[LEVEL]),
            [r#""file:///etc/background.png" leaves the bundle"#]
        );
    }

    #[test]
    fn a_path_with_a_percent_sign_is_a_problem() {
        let styling = settings(BlockSettings::default().with_background("asset/50%.png"));

        assert_eq!(
            messages(&styling, &[LEVEL]),
            [r#""asset/50%.png" holds %, which an image path must avoid"#]
        );
    }

    #[test]
    fn only_plain_relative_components_stay_inside_the_bundle() {
        let inside = ["asset/knob.png", "knob.png", "asset/.hidden/...png"];
        let outside = [
            "",
            "asset/",
            "/asset",
            "./asset",
            "asset/./knob.png",
            "..",
            "a//b",
        ];

        assert!(inside.into_iter().all(|it| PathReason::of(it).is_none()));
        assert!(outside.into_iter().all(|it| PathReason::of(it).is_some()));
    }

    #[test]
    fn a_path_is_given_the_first_reason_that_applies() {
        assert_eq!(
            PathReason::of("../a b%.png"),
            Some(PathReason::LeavesBundle)
        );
        assert_eq!(PathReason::of("./a b%.png"), Some(PathReason::NotPlain));
        assert_eq!(PathReason::of("a b%.png"), Some(PathReason::NotIri));
        assert_eq!(PathReason::of("ab%.png"), Some(PathReason::Escaped));
    }

    #[test]
    fn a_problem_repeated_by_the_styling_is_reported_once() {
        let styling = settings(
            BlockSettings::default()
                .with_knob(Widget::new("asset/a b.png"))
                .with_list(Widget::new("asset/a b.png")),
        );

        assert_eq!(
            messages(&styling, &[LEVEL]),
            [r#""asset/a b.png" holds a character Turtle forbids in an IRI"#]
        );
    }

    #[test]
    fn an_overlap_names_the_first_shared_slot() {
        let styling = settings(
            BlockSettings::default()
                .with_parameter(Parameter::of(Params::LEVEL).with_pin(1))
                .with_parameter(
                    Parameter::of(Waves::WAVE)
                        .with_pin(0)
                        .with_style(HostStyle::ListWith2Slots),
                ),
        );

        assert_eq!(
            messages(&styling, Waves::CONTROLS),
            ["wave overlaps level in slot 1, counted from 0 like a pin"]
        );
    }

    #[derive(crabgram::Choice)]
    enum Wave {
        Sine,
        Square,
    }

    #[derive(Controls)]
    struct Waves {
        level: f32,
        wave: Wave,
    }

    #[test]
    fn a_block_image_of_no_control_is_a_problem() {
        let styling = Styling {
            block_image: Some(
                BlockImage::new("asset/block.png").with_animation(
                    Other::LFO,
                    Sprite::new("asset/lfo.png", Frame::horizontal(NonZeroU32::MIN))
                        .with_position(0, 50),
                ),
            ),
            ..Styling::default()
        };

        assert_eq!(
            messages(&styling, &[LEVEL]),
            ["the block image draws lfo, which is no control"]
        );
    }

    #[test]
    fn a_styled_hidden_control_is_a_problem() {
        #[derive(Controls)]
        struct Hidden {
            #[control(hidden)]
            secret: f32,
        }

        let styling = settings(BlockSettings::default().with_parameter(
            Parameter::of(Hidden::SECRET).with_widget(Widget::new("asset/knob.png")),
        ));

        assert_eq!(
            messages(&styling, Hidden::CONTROLS),
            ["secret is styled, but it is hidden"]
        );
    }

    #[test]
    fn a_scenes_background_of_a_control_scenes_leave_alone_is_a_problem() {
        #[derive(Controls)]
        struct Scenes {
            rate: f32,
            #[control(expensive)]
            size: f32,
            #[control(blockable)]
            mode: bool,
            #[control(output)]
            meter: f32,
        }

        let scenes = |control| {
            Parameter::of(control).with_widget(
                Widget::new("asset/knob.png").with_background_scenes("asset/scenes.png"),
            )
        };
        let styling = settings(
            BlockSettings::default()
                .with_parameter(scenes(Scenes::CONTROLS[0]))
                .with_parameter(scenes(Scenes::CONTROLS[1]))
                .with_parameter(scenes(Scenes::CONTROLS[2]))
                .with_parameter(scenes(Scenes::CONTROLS[3])),
        );

        assert_eq!(
            messages(&styling, Scenes::CONTROLS),
            ["size", "mode", "meter"].map(|symbol| format!(
                "{symbol} has a scenes background, but scenes leave it alone"
            ))
        );
    }

    #[derive(Controls)]
    struct States {
        rate: f32,
        #[control(blockable)]
        mode: bool,
        #[control(output)]
        meter: f32,
    }

    fn overlays() -> Overlays {
        Overlays::default()
            .with_blocked("asset/blocked.png")
            .with_inactive("asset/inactive.png")
    }

    #[test]
    fn a_blocked_overlay_of_a_control_that_is_not_blockable_is_a_problem() {
        let overlaid = |control| {
            Parameter::of(control)
                .with_widget(Widget::new("asset/knob.png").with_overlays(overlays()))
        };
        let styling = settings(
            BlockSettings::default()
                .with_parameter(overlaid(States::CONTROLS[0]))
                .with_parameter(overlaid(States::CONTROLS[1])),
        );

        assert_eq!(
            messages(&styling, States::CONTROLS),
            ["rate never shows its dgcs:blocked overlay"]
        );
    }

    #[test]
    fn any_overlay_of_an_output_is_a_problem() {
        let styling = settings(
            BlockSettings::default().with_parameter(
                Parameter::of(States::METER)
                    .with_widget(Widget::new("asset/meter.png").with_overlays(overlays())),
            ),
        );

        assert_eq!(
            messages(&styling, States::CONTROLS),
            [
                "meter never shows its dgcs:blocked overlay",
                "meter never shows its dgcs:inactive overlay"
            ]
        );
    }

    #[test]
    fn the_default_meter_is_checked_like_its_outputs_but_other_defaults_are_not() {
        let widget = |path| {
            Widget::new(path)
                .with_background_scenes("asset/scenes.png")
                .with_overlays(Overlays::default().with_inactive("asset/inactive.png"))
        };
        let styling = settings(
            BlockSettings::default()
                .with_knob(widget("asset/knob.png"))
                .with_toggle(widget("asset/toggle.png"))
                .with_meter(widget("asset/meter.png")),
        );

        assert_eq!(
            messages(&styling, States::CONTROLS),
            [
                "meter has a scenes background, but scenes leave it alone",
                "meter never shows its dgcs:inactive overlay"
            ]
        );
    }

    #[test]
    fn a_start_padding_past_the_limit_is_a_problem() {
        let styling = settings(BlockSettings::default().with_start_padding(6));

        assert_eq!(
            messages(&styling, &[LEVEL]),
            ["a start padding of 6 exceeds the limit of 5"]
        );
    }

    #[test]
    fn no_control_styled_is_a_problem() {
        let styling = settings(
            BlockSettings::default()
                .with_parameter(Parameter::of(Params::LEVEL).with_pin(1))
                .with_parameter(Parameter::of(Other::GAIN).with_pin(3)),
        );

        assert_eq!(
            messages(&styling, &[LEVEL]),
            ["gain is styled, but it is no control"]
        );
    }

    #[test]
    fn a_widget_across_two_pages_is_a_problem() {
        let styling = settings(
            BlockSettings::default().with_parameter(
                Parameter::of(Waves::WAVE)
                    .with_pin(5)
                    .with_style(HostStyle::ListWith2Slots),
            ),
        );

        assert_eq!(
            messages(&styling, Waves::CONTROLS),
            ["wave crosses from page 1 to the next"]
        );
    }
}
