//! Writing Turtle: [`Styling::turtle`] writes `styling.ttl`, [`FILE`].
//!
//! The hidden items write any Turtle; `crabgram-build` writes the plugin description with them.

use core::{
    fmt::{self, Write},
    iter,
};

use crabgram::WidgetKind;

use crate::{
    Animation, Axis, BlockImage, BlockSettings, Bypass, Frame, Image, Pagination, Parameter,
    Sprite, Styling, TopBar, Widget, host,
};

const fn default_widget_predicate(kind: WidgetKind) -> &'static str {
    match kind {
        WidgetKind::Knob => "dgcs:knob",
        WidgetKind::List => "dgcs:list",
        WidgetKind::Toggle => "dgcs:toggle",
        WidgetKind::Meter => "dgcs:meter",
    }
}

fn push_formatted(out: &mut String, arguments: fmt::Arguments<'_>) {
    out.write_fmt(arguments).expect("a String takes any text");
}

fn push_quoted(out: &mut String, text: &str) {
    out.push('"');

    for character in text.chars() {
        if let Some(escaped) = escape(character) {
            out.push('\\');
            out.push(escaped);
        } else {
            out.push(character);
        }
    }

    out.push('"');
}

fn push_joined<T>(
    out: &mut String,
    items: impl IntoIterator<Item = T>,
    separator: &str,
    mut push: impl FnMut(&mut String, T),
) {
    for (position, item) in items.into_iter().enumerate() {
        if position > 0 {
            out.push_str(separator);
        }

        push(out, item);
    }
}

/// A namespace prefix the Turtle files use, with the namespace IRI it stands for.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Prefix {
    name: &'static str,
    iri: &'static str,
}

impl Prefix {
    const DGCS: Self = Self::new(
        "dgcs",
        "http://www.darkglass.com/lv2/ns/lv2ext/custom-styling#",
    );
    /// `lv2:`.
    pub const LV2: Self = Self::new("lv2", "http://lv2plug.in/ns/lv2core#");

    /// `name:` for `iri`.
    #[must_use]
    pub const fn new(name: &'static str, iri: &'static str) -> Self {
        Self { name, iri }
    }
}

/// Aligned `@prefix` lines for `prefixes`.
#[doc(hidden)]
#[must_use]
pub fn prefixes(prefixes: &[Prefix]) -> String {
    let width = prefixes
        .iter()
        .map(|it| it.name.len() + 1)
        .max()
        .unwrap_or(0);

    prefixes.iter().fold(String::new(), |mut lines, prefix| {
        let label = format!("{}:", prefix.name);

        push_formatted(
            &mut lines,
            format_args!("@prefix {label:<width$} <{}> .\n", prefix.iri),
        );
        lines
    })
}

/// The statement about `subject`, one line per predicate.
#[doc(hidden)]
#[must_use]
pub fn statement(subject: &str, properties: impl IntoIterator<Item = Entry>) -> String {
    let mut text = format!("<{subject}>\n");

    push_joined(
        &mut text,
        properties,
        " ;\n",
        |text, (predicate, object)| {
            text.push_str("    ");
            text.push_str(predicate);
            text.push(' ');
            object.write(text, 1);
        },
    );
    text.push_str(" .\n");
    text
}

fn push_decimal(out: &mut String, value: f32) {
    assert!(value.is_finite(), "Turtle has no decimal for {value}");

    let text = value.to_string();

    out.push_str(&text);

    if !text.contains('.') {
        out.push_str(".0");
    }
}

const fn escape(character: char) -> Option<char> {
    match character {
        '"' => Some('"'),
        '\\' => Some('\\'),
        '\n' => Some('n'),
        '\r' => Some('r'),
        '\t' => Some('t'),
        _ => None,
    }
}

/// A Turtle object.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum Term {
    /// A prefixed name, such as `lv2:Plugin`.
    Name(&'static str),
    /// An IRI, written in angle brackets.
    Iri(String),
    /// A string, quoted and escaped.
    Literal(String),
    /// An integer.
    Integer(i64),
    /// A finite decimal, written with a fractional part and no exponent, so Turtle reads it as
    /// a decimal.
    Decimal(f32),
    /// A blank node of these properties, one line each.
    Node(Vec<Entry>),
    /// Objects of one predicate, comma-separated, at least one: [`entry`] leaves out a predicate
    /// without objects.
    List(Vec<Self>),
}

impl Term {
    /// A blank node of `classes`, its `a`, then `properties`.
    #[must_use]
    pub fn node(classes: &[&'static str], properties: impl IntoIterator<Item = Entry>) -> Self {
        Self::Node(
            iter::once(("a", Self::names(classes)))
                .chain(properties)
                .collect(),
        )
    }

    /// The prefixed `names`, comma-separated.
    #[must_use]
    pub fn names(names: &[&'static str]) -> Self {
        Self::List(names.iter().copied().map(Self::Name).collect())
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Self::Name(name) => out.push_str(name),
            Self::Iri(iri) => {
                out.push('<');
                out.push_str(iri);
                out.push('>');
            }
            Self::Literal(text) => push_quoted(out, text),
            Self::Integer(value) => push_formatted(out, format_args!("{value}")),
            Self::Decimal(value) => push_decimal(out, *value),
            Self::List(terms) => {
                assert!(!terms.is_empty(), "a Turtle list holds at least one object");
                push_joined(out, terms, " , ", |out, term| term.write(out, depth));
            }
            Self::Node(properties) => {
                let indent = "    ".repeat(depth);

                out.push_str("[\n");
                push_joined(out, properties, "\n", |out, (predicate, object)| {
                    push_formatted(out, format_args!("{indent}    {predicate} "));
                    object.write(out, depth + 1);
                    out.push_str(" ;");
                });
                push_formatted(out, format_args!("\n{indent}]"));
            }
        }
    }
}

/// A predicate and its object.
#[doc(hidden)]
pub type Entry = (&'static str, Term);

/// `predicate` with the object `term` makes of `value`, or nothing without a value: the one way
/// to leave a property out.
#[doc(hidden)]
pub fn entry<T>(
    predicate: &'static str,
    value: Option<T>,
    term: impl FnOnce(T) -> Term,
) -> Option<Entry> {
    Some((predicate, term(value?)))
}

/// `items`, unless there are none.
#[doc(hidden)]
#[must_use]
pub fn nonempty<T>(items: Vec<T>) -> Option<Vec<T>> {
    (!items.is_empty()).then_some(items)
}

/// The file [`Styling::turtle`] is written to, listed in `manifest.ttl`.
pub const FILE: &str = "styling.ttl";

fn image(image: &Image) -> Term {
    let nonzero = |offset: i32| (offset != 0).then_some(offset);
    let (x, y) = image.offset;

    Term::node(
        &["dgcs:Image"],
        iter::once(("dgcs:path", Term::Iri(image.path.clone())))
            .chain(entry("dgcs:alignment", image.alignment, |it| {
                Term::Name(it.curie())
            }))
            .chain(entry("dgcs:offsetX", nonzero(x), |it| {
                Term::Integer(it.into())
            }))
            .chain(entry("dgcs:offsetY", nonzero(y), |it| {
                Term::Integer(it.into())
            })),
    )
}

fn images<'a>(
    images: impl IntoIterator<Item = (&'static str, &'a Option<Image>)>,
) -> impl Iterator<Item = Entry> {
    images
        .into_iter()
        .filter_map(|(predicate, it)| entry(predicate, it.as_ref(), image))
}

fn overlay(path: &str) -> Term {
    Term::node(
        &["dgcs:Overlay"],
        [("dgcs:path", Term::Iri(path.to_owned()))],
    )
}

fn frame(frame: Frame) -> Entry {
    let predicate = match frame.axis {
        Axis::Horizontal => "dgcs:width",
        Axis::Vertical => "dgcs:height",
    };

    (predicate, Term::Integer(frame.size.into()))
}

fn sprite(sprite: &Sprite) -> [Entry; 4] {
    [
        ("dgcs:path", Term::Iri(sprite.path.clone())),
        ("dgcs:x", Term::Integer(sprite.position.0.into())),
        ("dgcs:y", Term::Integer(sprite.position.1.into())),
        frame(sprite.frame),
    ]
}

fn widget(widget: &Widget) -> impl Iterator<Item = Entry> + '_ {
    images([
        ("dgcs:background", &widget.background),
        ("dgcs:backgroundScenes", &widget.background_scenes),
    ])
    .chain([("dgcs:control", image(&widget.control))])
    .chain(
        widget
            .overlays
            .paths()
            .map(|(predicate, path)| (predicate, overlay(path))),
    )
}

fn parameter(parameter: &Parameter) -> Term {
    Term::node(
        &["dgcs:BlockSettingsParameterWidget"],
        iter::once(("lv2:symbol", Term::Literal(parameter.symbol.clone())))
            .chain(entry("lv2:index", parameter.pin, |it| {
                Term::Integer(it.into())
            }))
            .chain(entry("dgcs:style", parameter.style(), |it| {
                Term::Name(it.curie())
            }))
            .chain(parameter.widget().into_iter().flat_map(widget)),
    )
}

/// The top bar, with the pagination button when there is one.
fn top_bar(bar: Option<&TopBar>, button: Option<&Image>) -> Option<Term> {
    let block_name = entry(
        "dgcs:topBarBlockName",
        bar.and_then(|it| it.block_name.as_ref()),
        |it| {
            Term::node(
                &["dgcs:BlockSettingsTopBarBlockName"],
                [("dgcs:background", image(it))],
            )
        },
    );
    let buttons = entry(
        "dgcs:topBarButtons",
        bar.map(|it| &it.buttons).filter(|it| !it.is_empty()),
        |buttons| {
            Term::node(
                &["dgcs:BlockSettingsTopBarButtons"],
                buttons
                    .iter()
                    .map(|(button, path)| (button.curie(), Term::Iri(path.clone()))),
            )
        },
    );
    let scene_control = entry(
        "dgcs:topBarSceneControl",
        bar.and_then(|it| it.scene_control.as_ref()),
        |it| {
            Term::node(
                &["dgcs:BlockSettingsTopBarSceneControl"],
                [
                    ("dgcs:sceneControlActiveScene", image(&it.active_scene)),
                    ("dgcs:sceneControlAllScenes", image(&it.all_scenes)),
                ],
            )
        },
    );

    (bar.is_some() || button.is_some()).then(|| {
        Term::node(
            &["dgcs:BlockSettingsTopBar"],
            entry(
                "dgcs:background",
                bar.and_then(|it| it.background.as_ref()),
                image,
            )
            .into_iter()
            .chain(block_name)
            .chain(buttons)
            .chain(scene_control)
            .chain(entry("dgcs:paginationButton", button, image)),
        )
    })
}

fn bypass(bypass: &Bypass) -> Term {
    let control = entry("dgcs:control", bypass.control.as_ref(), |path| {
        Term::node(
            &["dgcs:BlockSettingsBypassControlImage"],
            [
                ("dgcs:path", Term::Iri(path.clone())),
                frame(Frame {
                    axis: Axis::Horizontal,
                    size: host::BYPASS.0,
                }),
            ],
        )
    });

    Term::node(
        &["dgcs:BlockSettingsBypass"],
        images([
            ("dgcs:background", &bypass.background),
            ("dgcs:backgroundScenes", &bypass.background_scenes),
        ])
        .chain(control),
    )
}

fn block_settings(settings: &BlockSettings) -> Term {
    let padding = (settings.start_padding > 0).then_some(settings.start_padding);
    let defaults = settings.defaults().map(|(kind, it)| {
        (
            default_widget_predicate(kind),
            Term::node(&["dgcs:BlockSettingsDefaultWidget"], widget(it)),
        )
    });
    let (dots, button) = match &settings.pagination {
        Some(Pagination::Dots(image)) => (Some(image), None),
        Some(Pagination::Button(image)) => (None, Some(image)),
        None => (None, None),
    };
    let styled: Vec<Term> = settings.parameters.iter().map(parameter).collect();

    Term::node(
        &["dgcs:BlockSettings"],
        images([
            ("dgcs:about", &settings.about),
            ("dgcs:background", &settings.background),
        ])
        .chain(entry("dgcs:paginationDots", dots, image))
        .chain(top_bar(settings.top_bar.as_ref(), button).map(|it| ("dgcs:topBar", it)))
        .chain(entry("dgcs:bypass", settings.bypass.as_ref(), bypass))
        .chain(entry("dgcs:parameterStartPadding", padding, |it| {
            Term::Integer(it.into())
        }))
        .chain(defaults)
        .chain(entry("dgcs:parameters", nonempty(styled), Term::List)),
    )
}

fn block_image(block: &BlockImage) -> Term {
    let bypass = entry("dgcs:bypass", block.bypass.as_ref(), |it| {
        Term::node(&["dgcs:BlockImageBypass"], sprite(it))
    });
    let animations = block
        .animations
        .iter()
        .map(|Animation { symbol, sprite: it }| {
            Term::node(
                &["dgcs:BlockImageParameter"],
                iter::once(("lv2:symbol", Term::Literal(symbol.clone()))).chain(sprite(it)),
            )
        })
        .collect();

    Term::node(
        &["dgcs:BlockImage"],
        iter::once(("dgcs:path", Term::Iri(block.path.clone())))
            .chain(bypass)
            .chain(entry("dgcs:parameters", nonempty(animations), Term::List)),
    )
}

impl Styling {
    /// The `dgcs:` styling of plugin `uri` for [`FILE`], or `None` without a block image or block
    /// settings.
    ///
    /// [`Icons`](crate::Icons) go in the main Turtle file and are left out.
    ///
    /// ```
    /// use crabgram_styling::{Alignment, BlockSettings, Image, Pagination, Styling};
    ///
    /// let styling = Styling::default().with_block_settings(
    ///     BlockSettings::default().with_pagination(Pagination::Dots(
    ///         Image::new("asset/dots.png")
    ///             .with_alignment(Alignment::BottomMid)
    ///             .with_offset(0, -4),
    ///     )),
    /// );
    ///
    /// assert!(
    ///     styling
    ///         .turtle("urn:example:gain")
    ///         .unwrap()
    ///         .contains("dgcs:offsetY -4 ;")
    /// );
    /// assert_eq!(Styling::default().turtle("urn:example:gain"), None);
    /// ```
    #[must_use]
    pub fn turtle(&self, uri: &str) -> Option<String> {
        (self.block_image.is_some() || self.block_settings.is_some())
            .then(|| self.write_turtle(uri))
    }

    fn write_turtle(&self, uri: &str) -> String {
        let properties = [
            entry("dgcs:blockImage", self.block_image.as_ref(), block_image),
            entry(
                "dgcs:blockSettings",
                self.block_settings.as_ref(),
                block_settings,
            ),
        ]
        .into_iter()
        .flatten();

        format!(
            "{}\n{}",
            prefixes(&[Prefix::DGCS, Prefix::LV2]),
            statement(uri, properties)
        )
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use crabgram::{Choice, Controls, WidgetKind};

    use super::{
        Term, default_widget_predicate, entry, nonempty, push_decimal, push_quoted, statement,
    };
    use crate::{
        Alignment, BlockImage, BlockSettings, Button, Bypass, Frame, HostStyle, Image, Overlays,
        Pagination, Parameter, Sprite, Styling, TopBar, Widget,
    };

    fn size(size: u32) -> NonZeroU32 {
        NonZeroU32::new(size).unwrap()
    }

    #[derive(Choice)]
    enum Shape {
        Sine,
        Square,
    }

    #[derive(Controls)]
    struct Params {
        sync: bool,
        rate: f32,
        wave: Shape,
        #[control(output)]
        lfo: f32,
    }

    #[test]
    fn a_default_widget_predicate_names_the_widget() {
        for kind in [
            WidgetKind::Knob,
            WidgetKind::List,
            WidgetKind::Toggle,
            WidgetKind::Meter,
        ] {
            assert_eq!(
                default_widget_predicate(kind),
                format!("dgcs:{}", kind.name())
            );
        }
    }

    #[test]
    fn a_literal_escapes_quotes_backslashes_and_line_breaks() {
        let mut quoted = String::new();

        push_quoted(&mut quoted, "say \"hi\"\\\n");

        assert_eq!(quoted, r#""say \"hi\"\\\n""#);
    }

    #[test]
    fn a_decimal_has_a_fractional_part_and_no_exponent() {
        let written = |value: f32| {
            let mut text = String::new();

            push_decimal(&mut text, value);
            text
        };

        assert_eq!(written(1.0), "1.0");
        assert_eq!(written(-0.25), "-0.25");
        assert_eq!(written(1e-7), "0.0000001");
        assert_eq!(written(1e20), "100000000000000000000.0");
    }

    #[test]
    #[should_panic(expected = "Turtle has no decimal for NaN")]
    fn a_decimal_that_is_not_a_number_is_refused() {
        push_decimal(&mut String::new(), f32::NAN);
    }

    #[test]
    fn a_property_without_a_value_is_left_out() {
        let written = statement(
            "urn:example:x",
            entry("doap:license", nonempty(Vec::new()), Term::List)
                .into_iter()
                .chain([("doap:name", Term::Literal("X".to_owned()))]),
        );

        assert_eq!(written, "<urn:example:x>\n    doap:name \"X\" .\n");
    }

    #[test]
    #[should_panic(expected = "a Turtle list holds at least one object")]
    fn an_empty_list_is_refused() {
        let _ = statement("urn:example:x", [("doap:license", Term::List(Vec::new()))]);
    }

    #[test]
    fn a_parameter_that_says_nothing_is_left_out() {
        let styling = Styling {
            block_settings: Some(
                BlockSettings::default()
                    .with_parameter(Parameter::of(Params::SYNC))
                    .with_parameter(Parameter::of(Params::RATE).with_pin(2)),
            ),
            ..Styling::default()
        };
        let written = styling
            .turtle("urn:example:x")
            .expect("block settings make a styling file");

        assert!(!written.contains("\"sync\""));
        assert!(written.contains("\"rate\""));
    }

    #[test]
    fn a_style_set_after_images_writes_no_image() {
        let styling = Styling::default().with_block_settings(
            BlockSettings::default().with_parameter(
                Parameter::of(Params::WAVE)
                    .with_widget(
                        Widget::new("asset/wave.png")
                            .with_overlays(Overlays::default().with_blocked("asset/blocked.png")),
                    )
                    .with_style(HostStyle::ListWith2Slots),
            ),
        );
        let written = styling.turtle("urn:example:x").unwrap();

        assert!(written.contains("dgcs:style dgcs:listWith2Slots"));
        assert!(!written.contains("dgcs:blocked"));
        assert!(!written.contains("dgcs:control"));
    }

    #[test]
    fn a_pagination_button_goes_in_the_top_bar() {
        let styling = Styling::default().with_block_settings(
            BlockSettings::default().with_pagination(Pagination::Button("asset/pages.png".into())),
        );
        let written = styling.turtle("urn:example:x").unwrap();

        assert!(written.ends_with(
            "        dgcs:topBar [\n            a dgcs:BlockSettingsTopBar ;\n            \
             dgcs:paginationButton [\n                a dgcs:Image ;\n                dgcs:path \
             <asset/pages.png> ;\n            ] ;\n        ] ;\n    ] .\n"
        ));
        assert!(!written.contains("dgcs:paginationDots"));
    }

    #[test]
    fn the_whole_styling_nests_every_node() {
        let styling = Styling {
            icons: None,
            block_image: Some(
                BlockImage::new("asset/block.png")
                    .with_bypass(
                        Sprite::new("asset/block-bypass.png", Frame::horizontal(size(30)))
                            .with_position(158, 12),
                    )
                    .with_animation(
                        Params::LFO,
                        Sprite::new("asset/block-lfo.png", Frame::horizontal(size(60)))
                            .with_position(70, 50),
                    ),
            ),
            block_settings: Some(
                BlockSettings::default()
                    .with_about("asset/about.png")
                    .with_background("asset/background.png")
                    .with_pagination(Pagination::Dots(
                        Image::new("asset/dots.png").with_alignment(Alignment::BottomMid),
                    ))
                    .with_top_bar(
                        TopBar::default()
                            .with_background("asset/top-bar.png")
                            .with_block_name("asset/block-name.png")
                            .with_button(Button::Close, "asset/close.png")
                            .with_scene_control("asset/active.png", "asset/all.png"),
                    )
                    .with_bypass(Bypass::default().with_control("asset/bypass.png"))
                    .with_start_padding(1)
                    .with_knob(
                        Widget::new("asset/knob.png")
                            .with_background("asset/panel.png")
                            .with_overlays(Overlays {
                                blocked: Some("asset/blocked.png".into()),
                                ..Overlays::default()
                            }),
                    )
                    .with_parameter(
                        Parameter::of(Params::WAVE)
                            .with_pin(2)
                            .with_style(HostStyle::ListWith2Slots),
                    ),
            ),
        };

        assert_eq!(styling.turtle("urn:example:lfo").unwrap(), GOLDEN);
    }

    const GOLDEN: &str = r#"@prefix dgcs: <http://www.darkglass.com/lv2/ns/lv2ext/custom-styling#> .
@prefix lv2:  <http://lv2plug.in/ns/lv2core#> .

<urn:example:lfo>
    dgcs:blockImage [
        a dgcs:BlockImage ;
        dgcs:path <asset/block.png> ;
        dgcs:bypass [
            a dgcs:BlockImageBypass ;
            dgcs:path <asset/block-bypass.png> ;
            dgcs:x 158 ;
            dgcs:y 12 ;
            dgcs:width 30 ;
        ] ;
        dgcs:parameters [
            a dgcs:BlockImageParameter ;
            lv2:symbol "lfo" ;
            dgcs:path <asset/block-lfo.png> ;
            dgcs:x 70 ;
            dgcs:y 50 ;
            dgcs:width 60 ;
        ] ;
    ] ;
    dgcs:blockSettings [
        a dgcs:BlockSettings ;
        dgcs:about [
            a dgcs:Image ;
            dgcs:path <asset/about.png> ;
        ] ;
        dgcs:background [
            a dgcs:Image ;
            dgcs:path <asset/background.png> ;
        ] ;
        dgcs:paginationDots [
            a dgcs:Image ;
            dgcs:path <asset/dots.png> ;
            dgcs:alignment dgcs:alignBottomMid ;
        ] ;
        dgcs:topBar [
            a dgcs:BlockSettingsTopBar ;
            dgcs:background [
                a dgcs:Image ;
                dgcs:path <asset/top-bar.png> ;
            ] ;
            dgcs:topBarBlockName [
                a dgcs:BlockSettingsTopBarBlockName ;
                dgcs:background [
                    a dgcs:Image ;
                    dgcs:path <asset/block-name.png> ;
                ] ;
            ] ;
            dgcs:topBarButtons [
                a dgcs:BlockSettingsTopBarButtons ;
                dgcs:buttonClose <asset/close.png> ;
            ] ;
            dgcs:topBarSceneControl [
                a dgcs:BlockSettingsTopBarSceneControl ;
                dgcs:sceneControlActiveScene [
                    a dgcs:Image ;
                    dgcs:path <asset/active.png> ;
                ] ;
                dgcs:sceneControlAllScenes [
                    a dgcs:Image ;
                    dgcs:path <asset/all.png> ;
                ] ;
            ] ;
        ] ;
        dgcs:bypass [
            a dgcs:BlockSettingsBypass ;
            dgcs:control [
                a dgcs:BlockSettingsBypassControlImage ;
                dgcs:path <asset/bypass.png> ;
                dgcs:width 107 ;
            ] ;
        ] ;
        dgcs:parameterStartPadding 1 ;
        dgcs:knob [
            a dgcs:BlockSettingsDefaultWidget ;
            dgcs:background [
                a dgcs:Image ;
                dgcs:path <asset/panel.png> ;
            ] ;
            dgcs:control [
                a dgcs:Image ;
                dgcs:path <asset/knob.png> ;
            ] ;
            dgcs:blocked [
                a dgcs:Overlay ;
                dgcs:path <asset/blocked.png> ;
            ] ;
        ] ;
        dgcs:parameters [
            a dgcs:BlockSettingsParameterWidget ;
            lv2:symbol "wave" ;
            lv2:index 2 ;
            dgcs:style dgcs:listWith2Slots ;
        ] ;
    ] .
"#;
}
