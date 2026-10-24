use std::collections::BTreeMap;

use crabgram::{Control, Controls, Field, WidgetKind, access::Output};

use crate::{Axis, Image, Sprite};

/// Builder methods: `with_<field>(values)` returns the value with the field set. Without a doc,
/// one linking the getter of the field is written.
macro_rules! setters {
    () => {};
    ($name:ident($($value:ident: $ty:ty),+) => $field:ident = $set:expr; $($rest:tt)*) => {
        setters! {
            #[doc = concat!("Sets [`", stringify!($field), "`](Self::", stringify!($field), "()).")]
            $name($($value: $ty),+) => $field = $set;
            $($rest)*
        }
    };
    ($(#[$doc:meta])+ $name:ident($($value:ident: $ty:ty),+) => $field:ident = $set:expr; $($rest:tt)*) => {
        $(#[$doc])+
        #[must_use]
        pub fn $name(self, $($value: $ty),+) -> Self {
            Self { $field: $set, ..self }
        }

        setters! { $($rest)* }
    };
}

/// Read methods named after their fields, `const` where marked.
macro_rules! getters {
    () => {};
    ($(#[$doc:meta])* const $name:ident -> $ty:ty = |$this:ident| $get:expr; $($rest:tt)*) => {
        $(#[$doc])*
        #[must_use]
        pub const fn $name(&self) -> $ty {
            let $this = self;

            $get
        }

        getters! { $($rest)* }
    };
    ($(#[$doc:meta])* $name:ident -> $ty:ty = |$this:ident| $get:expr; $($rest:tt)*) => {
        $(#[$doc])*
        #[must_use]
        pub fn $name(&self) -> $ty {
            let $this = self;

            $get
        }

        getters! { $($rest)* }
    };
}

pub(crate) use getters;
pub(crate) use setters;

/// Still block images, `dg:blockImageOn` and `dg:blockImageOff`.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Icons {
    pub(crate) on: String,
    pub(crate) off: String,
}

impl Icons {
    /// The block while on, then while bypassed.
    #[must_use]
    pub fn new(on: impl Into<String>, off: impl Into<String>) -> Self {
        Self {
            on: on.into(),
            off: off.into(),
        }
    }

    getters! {
        /// Block while on.
        const on -> &str = |it| it.on.as_str();

        /// Block while bypassed.
        const off -> &str = |it| it.off.as_str();
    }
}

/// `dgcs:blockImage`: the block image, with sprites that follow controls.
///
/// Every sprite frame stays inside the [`BLOCK_IMAGE`](crate::host::BLOCK_IMAGE) square.
///
/// ```
/// use std::num::NonZeroU32;
///
/// use crabgram::Controls;
/// use crabgram_styling::{BlockImage, Frame, Sprite};
///
/// #[derive(Controls)]
/// struct Params {
///     #[control(output)]
///     lfo: f32,
/// }
///
/// let frame = Frame::horizontal(NonZeroU32::new(60).unwrap());
/// let block = BlockImage::new("asset/block.png").with_animation(
///     Params::LFO,
///     Sprite::new("asset/block-lfo.png", frame).with_position(70, 50),
/// );
///
/// assert_eq!(block.animations()[0].symbol(), "lfo");
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockImage {
    pub(crate) path: String,
    pub(crate) bypass: Option<Sprite>,
    pub(crate) animations: Vec<Animation>,
}

/// `dgcs:BlockImageParameter`: a sprite of the block image that follows an output.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Animation {
    pub(crate) symbol: String,
    pub(crate) sprite: Sprite,
}

impl Animation {
    getters! {
        /// Symbol of the output.
        const symbol -> &str = |it| it.symbol.as_str();

        /// Its frames and their place in the block image.
        const sprite -> &Sprite = |it| &it.sprite;
    }
}

impl BlockImage {
    /// Nothing drawn on it yet.
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            bypass: None,
            animations: Vec::new(),
        }
    }

    getters! {
        /// One frame, or two side by side: on, then off.
        const path -> &str = |it| it.path.as_str();

        /// Two frames: on, then off.
        const bypass -> Option<&Sprite> = |it| it.bypass.as_ref();

        /// Sprites that follow output controls.
        const animations -> &[Animation] = |it| it.animations.as_slice();
    }

    setters! {
        with_bypass(sprite: Sprite) => bypass = Some(sprite);
    }

    /// Animates `sprite` by the value of `output`.
    #[must_use]
    pub fn with_animation<C: Controls, V>(
        mut self,
        output: Field<C, Output, V>,
        sprite: Sprite,
    ) -> Self {
        self.animations.push(Animation {
            symbol: output.as_ref().symbol().to_owned(),
            sprite,
        });
        self
    }

    /// Every sprite drawn on it: the bypass, then the animations.
    pub fn sprites(&self) -> impl Iterator<Item = &Sprite> {
        self.bypass
            .iter()
            .chain(self.animations.iter().map(|it| &it.sprite))
    }
}

/// Images laid over a widget while [`Context::set_state`](crabgram::Context::set_state) holds a
/// state.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Overlays {
    pub(crate) blocked: Option<String>,
    pub(crate) inactive: Option<String>,
}

impl Overlays {
    getters! {
        /// `dgcs:blocked`, while [`ControlState::Blocked`](crabgram::ControlState::Blocked).
        blocked -> Option<&str> = |it| it.blocked.as_deref();

        /// `dgcs:inactive`, while [`Activity::Inactive`](crabgram::Activity::Inactive), or
        /// [`ControlState::Inactive`](crabgram::ControlState::Inactive) for a blockable control.
        inactive -> Option<&str> = |it| it.inactive.as_deref();
    }

    setters! {
        with_blocked(path: impl Into<String>) => blocked = Some(path.into());
        with_inactive(path: impl Into<String>) => inactive = Some(path.into());
    }

    /// The set overlays as Turtle predicate and path.
    pub(crate) fn paths(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            ("dgcs:blocked", &self.blocked),
            ("dgcs:inactive", &self.inactive),
        ]
        .into_iter()
        .filter_map(|(predicate, path)| Some((predicate, path.as_deref()?)))
    }
}

/// Images of a default or parameter widget: control frames, and a background and overlays
/// around them.
///
/// A default widget styles every parameter of its kind; a [`Parameter`] wins for one port.
///
/// ```
/// use crabgram_styling::Widget;
///
/// let knob = Widget::new("asset/knob.png").with_background("asset/panel.png");
///
/// assert_eq!(knob.control().path(), "asset/knob.png");
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Widget {
    pub(crate) background: Option<Image>,
    pub(crate) background_scenes: Option<Image>,
    pub(crate) control: Image,
    pub(crate) overlays: Overlays,
}

impl Widget {
    /// The control frames `control`, without a background or overlays.
    #[must_use]
    pub fn new(control: impl Into<Image>) -> Self {
        Self {
            background: None,
            background_scenes: None,
            control: control.into(),
            overlays: Overlays::default(),
        }
    }

    getters! {
        /// Panel behind the control.
        const background -> Option<&Image> = |it| it.background.as_ref();

        /// Panel while the parameter follows scenes.
        const background_scenes -> Option<&Image> = |it| it.background_scenes.as_ref();

        /// Square frames, minimum first; the crate docs list the counts.
        const control -> &Image = |it| &it.control;

        /// Layers for special states.
        const overlays -> &Overlays = |it| &it.overlays;
    }

    setters! {
        with_background(image: impl Into<Image>) => background = Some(image.into());
        with_background_scenes(image: impl Into<Image>) => background_scenes = Some(image.into());
        with_overlays(overlays: Overlays) => overlays = overlays;
    }
}

/// `dgcs:style`: a stock widget instead of images, set by [`Parameter::with_style`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum HostStyle {
    /// `dgcs:fader`, for a knob.
    Fader,
    /// `dgcs:listWith2Slots`, for a list.
    ListWith2Slots,
    /// `dgcs:listWith3Slots`, for a list.
    ListWith3Slots,
}

impl HostStyle {
    pub(crate) const fn curie(self) -> &'static str {
        match self {
            Self::Fader => "dgcs:fader",
            Self::ListWith2Slots => "dgcs:listWith2Slots",
            Self::ListWith3Slots => "dgcs:listWith3Slots",
        }
    }

    /// Slots the widget takes.
    #[must_use]
    pub const fn slots(self) -> u32 {
        match self {
            Self::Fader => 1,
            Self::ListWith2Slots => 2,
            Self::ListWith3Slots => 3,
        }
    }

    /// Kind of control the style works on.
    #[must_use]
    pub const fn kind(self) -> WidgetKind {
        match self {
            Self::Fader => WidgetKind::Knob,
            Self::ListWith2Slots | Self::ListWith3Slots => WidgetKind::List,
        }
    }
}

/// `dgcs:BlockSettingsParameterWidget`: the look of one control, a stock widget or images, and
/// its pin.
///
/// ```
/// use crabgram::{Choice, Controls};
/// use crabgram_styling::{HostStyle, Parameter, Widget};
///
/// #[derive(Choice)]
/// enum Shape {
///     Sine,
///     Square,
/// }
///
/// #[derive(Controls)]
/// struct Params {
///     wave: Shape,
///     #[control(output)]
///     meter: f32,
/// }
///
/// let wave = Parameter::of(Params::WAVE).with_style(HostStyle::ListWith3Slots);
/// let meter = Parameter::of(Params::METER)
///     .with_pin(23)
///     .with_widget(Widget::new("asset/meter.png"));
///
/// assert_eq!(wave.slots(), 3);
/// assert_eq!(meter.pin(), Some(23));
/// assert!(meter.style().is_none());
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Parameter {
    pub(crate) symbol: String,
    pub(crate) pin: Option<u32>,
    look: Option<Look>,
}

/// How a [`Parameter`] is drawn.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Look {
    Stock(HostStyle),
    Images(Widget),
}

impl Parameter {
    /// Without a look and unpinned.
    #[must_use]
    pub fn of(control: impl AsRef<Control>) -> Self {
        Self {
            symbol: control.as_ref().symbol().to_owned(),
            pin: None,
            look: None,
        }
    }

    getters! {
        /// Symbol of the styled control.
        const symbol -> &str = |it| it.symbol.as_str();

        /// `lv2:index`: pinned slot from 0 across pages, padding included (not the port index).
        const pin -> Option<u32> = |it| it.pin;

        /// The stock widget, from [`with_style`](Self::with_style).
        const style -> Option<HostStyle> = |it| match &it.look {
            Some(Look::Stock(style)) => Some(*style),
            Some(Look::Images(_)) | None => None,
        };

        /// The images, from [`with_widget`](Self::with_widget); they win over the default widget
        /// of its kind.
        const widget -> Option<&Widget> = |it| match &it.look {
            Some(Look::Images(widget)) => Some(widget),
            Some(Look::Stock(_)) | None => None,
        };
    }

    setters! {
        /// Pins the widget to `slot`, from 0 across pages, padding included.
        with_pin(slot: u32) => pin = Some(slot);

        /// Draws the widget as the stock `style`, in place of any images.
        with_style(style: HostStyle) => look = Some(Look::Stock(style));

        /// Draws the widget from the images of `widget`, in place of any style.
        with_widget(widget: Widget) => look = Some(Look::Images(widget));
    }

    /// Slots the widget takes: those of its style, else one.
    #[must_use]
    pub const fn slots(&self) -> u32 {
        match &self.look {
            Some(Look::Stock(style)) => style.slots(),
            Some(Look::Images(_)) | None => 1,
        }
    }
}

/// A top bar button.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Button {
    /// `dgcs:buttonBack`.
    Back,
    /// `dgcs:buttonClose`.
    Close,
    /// `dgcs:buttonMore`.
    More,
    /// `dgcs:buttonRemove`.
    Remove,
    /// `dgcs:buttonSwap`.
    Swap,
}

impl Button {
    /// Every button.
    pub const ALL: [Self; 5] = [
        Self::Back,
        Self::Close,
        Self::More,
        Self::Remove,
        Self::Swap,
    ];

    pub(crate) const fn curie(self) -> &'static str {
        match self {
            Self::Back => "dgcs:buttonBack",
            Self::Close => "dgcs:buttonClose",
            Self::More => "dgcs:buttonMore",
            Self::Remove => "dgcs:buttonRemove",
            Self::Swap => "dgcs:buttonSwap",
        }
    }

    /// Lower case, for file names.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Back => "back",
            Self::Close => "close",
            Self::More => "more",
            Self::Remove => "remove",
            Self::Swap => "swap",
        }
    }

    /// Size of one of its two frames.
    ///
    /// ```
    /// assert_eq!(crabgram_styling::Button::Close.frame(), (85, 50));
    /// ```
    #[must_use]
    pub const fn frame(self) -> (u32, u32) {
        match self {
            Self::Back | Self::Close => (85, 50),
            Self::More | Self::Remove | Self::Swap => (50, 50),
        }
    }

    /// The direction its two frames run: side by side, unless a frame is wider than tall.
    #[must_use]
    pub const fn axis(self) -> Axis {
        let (width, height) = self.frame();

        if width <= height {
            Axis::Horizontal
        } else {
            Axis::Vertical
        }
    }
}

/// `dgcs:topBarSceneControl`: one image per scene mode.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SceneControl {
    pub(crate) active_scene: Image,
    pub(crate) all_scenes: Image,
}

impl SceneControl {
    getters! {
        /// `dgcs:sceneControlActiveScene`.
        const active_scene -> &Image = |it| &it.active_scene;

        /// `dgcs:sceneControlAllScenes`.
        const all_scenes -> &Image = |it| &it.all_scenes;
    }
}

/// `dgcs:topBar`.
///
/// ```
/// use crabgram_styling::{Button, TopBar};
///
/// let bar = TopBar::default().with_button(Button::Close, "asset/button-close.png");
///
/// assert_eq!(bar.buttons()[&Button::Close], "asset/button-close.png");
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TopBar {
    pub(crate) background: Option<Image>,
    pub(crate) block_name: Option<Image>,
    pub(crate) buttons: BTreeMap<Button, String>,
    pub(crate) scene_control: Option<SceneControl>,
}

impl TopBar {
    getters! {
        /// A [`TOP_BAR`](crate::host::TOP_BAR) image.
        const background -> Option<&Image> = |it| it.background.as_ref();

        /// `dgcs:topBarBlockName`, a [`BLOCK_NAME`](crate::host::BLOCK_NAME) image.
        const block_name -> Option<&Image> = |it| it.block_name.as_ref();

        /// Button images, each exactly its [`Button::framing`].
        const buttons -> &BTreeMap<Button, String> = |it| &it.buttons;

        /// `dgcs:topBarSceneControl`.
        const scene_control -> Option<&SceneControl> = |it| it.scene_control.as_ref();
    }

    setters! {
        with_background(image: impl Into<Image>) => background = Some(image.into());
        with_block_name(image: impl Into<Image>) => block_name = Some(image.into());
        with_scene_control(active_scene: impl Into<Image>, all_scenes: impl Into<Image>) =>
            scene_control = Some(SceneControl {
                active_scene: active_scene.into(),
                all_scenes: all_scenes.into(),
            });
    }

    /// Sets the image of `button`.
    #[must_use]
    pub fn with_button(mut self, button: Button, path: impl Into<String>) -> Self {
        self.buttons.insert(button, path.into());
        self
    }
}

/// How the block settings show the pages, one frame per page: dots or a button.
///
/// ```
/// use crabgram_styling::{BlockSettings, Pagination};
///
/// let settings =
///     BlockSettings::default().with_pagination(Pagination::Dots("asset/dots.png".into()));
///
/// assert!(matches!(settings.pagination(), Some(Pagination::Dots(_))));
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Pagination {
    /// `dgcs:paginationDots`: frames side by side.
    Dots(Image),
    /// `dgcs:paginationButton` in the top bar: frames along the long side.
    Button(Image),
}

/// `dgcs:bypass`.
///
/// ```
/// use crabgram_styling::Bypass;
///
/// let bypass = Bypass::default().with_control("asset/bypass.png");
/// assert!(bypass.control().is_some());
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bypass {
    pub(crate) background: Option<Image>,
    pub(crate) background_scenes: Option<Image>,
    pub(crate) control: Option<String>,
}

impl Bypass {
    getters! {
        /// Behind the switch.
        const background -> Option<&Image> = |it| it.background.as_ref();

        /// Behind the switch while it follows scenes.
        const background_scenes -> Option<&Image> = |it| it.background_scenes.as_ref();

        /// Two [`BYPASS`](crate::host::BYPASS)-sized frames side by side at this path, on then off.
        control -> Option<&str> = |it| it.control.as_deref();
    }

    setters! {
        with_background(image: impl Into<Image>) => background = Some(image.into());
        with_background_scenes(image: impl Into<Image>) => background_scenes = Some(image.into());
        with_control(path: impl Into<String>) => control = Some(path.into());
    }
}

/// `dgcs:blockSettings`.
///
/// ```
/// use crabgram::{Choice, Controls};
/// use crabgram_styling::{BlockSettings, HostStyle, Parameter, Widget};
///
/// #[derive(Choice)]
/// enum Shape {
///     Sine,
///     Square,
/// }
///
/// #[derive(Controls)]
/// struct Params {
///     wave: Shape,
///     rate: f32,
/// }
///
/// let settings = BlockSettings::default()
///     .with_start_padding(2)
///     .with_knob(Widget::new("asset/knob.png"))
///     .with_parameter(Parameter::of(Params::WAVE).with_style(HostStyle::ListWith3Slots));
///
/// assert_eq!(settings.start_padding(), 2);
/// assert!(settings.parameter_of(Params::WAVE).is_some());
/// assert!(settings.parameter_of(Params::RATE).is_none());
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockSettings {
    pub(crate) background: Option<Image>,
    pub(crate) about: Option<Image>,
    pub(crate) pagination: Option<Pagination>,
    pub(crate) top_bar: Option<TopBar>,
    pub(crate) bypass: Option<Bypass>,
    pub(crate) start_padding: u32,
    pub(crate) knob: Option<Widget>,
    pub(crate) list: Option<Widget>,
    pub(crate) toggle: Option<Widget>,
    pub(crate) meter: Option<Widget>,
    pub(crate) parameters: Vec<Parameter>,
}

impl BlockSettings {
    getters! {
        /// One [`SCREEN`](crate::host::SCREEN) frame per page, side by side.
        const background -> Option<&Image> = |it| it.background.as_ref();

        /// An [`ABOUT`](crate::host::ABOUT) image.
        const about -> Option<&Image> = |it| it.about.as_ref();

        /// `dgcs:paginationDots` or `dgcs:paginationButton`.
        const pagination -> Option<&Pagination> = |it| it.pagination.as_ref();

        /// `dgcs:topBar`.
        const top_bar -> Option<&TopBar> = |it| it.top_bar.as_ref();

        /// `dgcs:bypass`.
        const bypass -> Option<&Bypass> = |it| it.bypass.as_ref();

        /// Empty slots before the first widget (`dgcs:parameterStartPadding`).
        const start_padding -> u32 = |it| it.start_padding;

        /// Every knob without a parameter widget of its own.
        const knob -> Option<&Widget> = |it| it.knob.as_ref();

        /// Every list without a parameter widget of its own.
        const list -> Option<&Widget> = |it| it.list.as_ref();

        /// Every switch without a parameter widget of its own.
        const toggle -> Option<&Widget> = |it| it.toggle.as_ref();

        /// Every meter without a parameter widget of its own.
        const meter -> Option<&Widget> = |it| it.meter.as_ref();

        /// One per styled control, in the order set; they win over the defaults.
        const parameters -> &[Parameter] = |it| it.parameters.as_slice();
    }

    setters! {
        with_background(image: impl Into<Image>) => background = Some(image.into());
        with_about(image: impl Into<Image>) => about = Some(image.into());
        with_pagination(pagination: Pagination) => pagination = Some(pagination);
        with_top_bar(top_bar: TopBar) => top_bar = Some(top_bar);
        with_bypass(bypass: Bypass) => bypass = Some(bypass);

        /// Leaves `slots` empty before the first widget, at most
        /// [`MAX_START_PADDING`](crate::host::MAX_START_PADDING).
        with_start_padding(slots: u32) => start_padding = slots;

        with_knob(widget: Widget) => knob = Some(widget);
        with_list(widget: Widget) => list = Some(widget);
        with_toggle(widget: Widget) => toggle = Some(widget);
        with_meter(widget: Widget) => meter = Some(widget);
    }

    /// Sets the look of one control, in place of an earlier one of the same symbol; one without a
    /// pin or a look removes it.
    #[must_use]
    pub fn with_parameter(mut self, parameter: Parameter) -> Self {
        let earlier = self
            .parameters
            .iter()
            .position(|it| it.symbol == parameter.symbol);

        match (earlier, parameter.pin.is_some() || parameter.look.is_some()) {
            (Some(index), true) => self.parameters[index] = parameter,
            (Some(index), false) => {
                self.parameters.remove(index);
            }
            (None, true) => self.parameters.push(parameter),
            (None, false) => {}
        }

        self
    }

    /// Background of the default widget of the kind drawn for `control`, by its style if any.
    ///
    /// ```
    /// use crabgram::Controls;
    /// use crabgram_styling::{BlockSettings, HostStyle, Parameter, Widget};
    ///
    /// #[derive(Controls)]
    /// struct Params {
    ///     gain: f32,
    /// }
    ///
    /// let fader = BlockSettings::default()
    ///     .with_parameter(Parameter::of(Params::GAIN).with_style(HostStyle::Fader));
    /// let knob = Widget::new("asset/knob.png").with_background("asset/panel.png");
    ///
    /// assert!(fader.stock_background(Params::GAIN).is_none());
    /// assert_eq!(
    ///     fader
    ///         .with_knob(knob)
    ///         .stock_background(Params::GAIN)
    ///         .map(|it| it.path()),
    ///     Some("asset/panel.png")
    /// );
    /// ```
    #[must_use]
    pub fn stock_background(&self, control: impl AsRef<Control>) -> Option<&Image> {
        let control = control.as_ref();
        let kind = self
            .parameter_of(control)
            .and_then(Parameter::style)
            .map_or_else(|| control.widget(), HostStyle::kind);

        self.default_widget(kind)?.background.as_ref()
    }

    /// The default widgets that are set, with their kind.
    pub(crate) fn defaults(&self) -> impl Iterator<Item = (WidgetKind, &Widget)> {
        [
            WidgetKind::Knob,
            WidgetKind::List,
            WidgetKind::Toggle,
            WidgetKind::Meter,
        ]
        .into_iter()
        .filter_map(|kind| Some((kind, self.default_widget(kind)?)))
    }

    const fn default_widget(&self, kind: WidgetKind) -> Option<&Widget> {
        match kind {
            WidgetKind::Knob => self.knob.as_ref(),
            WidgetKind::List => self.list.as_ref(),
            WidgetKind::Toggle => self.toggle.as_ref(),
            WidgetKind::Meter => self.meter.as_ref(),
        }
    }

    fn parameter_index(&self, control: &Control) -> Option<usize> {
        self.parameters
            .iter()
            .position(|it| it.symbol == control.symbol())
    }

    /// The look of `control`, if it has one of its own.
    #[must_use]
    pub fn parameter_of(&self, control: impl AsRef<Control>) -> Option<&Parameter> {
        Some(&self.parameters[self.parameter_index(control.as_ref())?])
    }

    /// The widget shown for `control` and where it comes from; `None` for a stock widget.
    pub(crate) fn sourced_widget_of(&self, control: &Control) -> Option<(WidgetSource, &Widget)> {
        let (source, widget) = self.parameter_index(control).map_or_else(
            || {
                let kind = control.widget();

                (WidgetSource::Default(kind), self.default_widget(kind))
            },
            |index| {
                (
                    WidgetSource::Parameter(index),
                    self.parameters[index].widget(),
                )
            },
        );

        widget.map(|it| (source, it))
    }

    /// The widget shown for `control`: the images of its [`Parameter`], or the default of its kind
    /// when it has none; `None` for a stock widget.
    #[must_use]
    pub fn widget_of(&self, control: impl AsRef<Control>) -> Option<&Widget> {
        self.sourced_widget_of(control.as_ref())
            .map(|(_, widget)| widget)
    }

    /// Every widget of the settings with where it comes from: the defaults, then the parameters.
    pub(crate) fn sourced_widgets(&self) -> impl Iterator<Item = (WidgetSource, &Widget)> {
        self.defaults()
            .map(|(kind, widget)| (WidgetSource::Default(kind), widget))
            .chain(
                self.parameters
                    .iter()
                    .enumerate()
                    .filter_map(|(index, it)| Some((WidgetSource::Parameter(index), it.widget()?))),
            )
    }
}

/// Where a widget of [`BlockSettings`] comes from: the default of a kind, or the parameter at an
/// index of [`BlockSettings::parameters`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidgetSource {
    Default(WidgetKind),
    Parameter(usize),
}

/// All styling of one plugin.
///
/// [`Styling::turtle`] writes it to its own file, listed in `manifest.ttl`.
///
/// ```
/// use crabgram_styling::{BlockSettings, Icons, Styling, Widget};
///
/// let styling = Styling::default()
///     .with_icons(Icons::new("asset/on.png", "asset/off.png"))
///     .with_block_settings(BlockSettings::default().with_knob(Widget::new("asset/knob.png")));
///
/// let turtle = styling.turtle("urn:example:gain").unwrap();
/// assert!(turtle.contains("dgcs:knob ["));
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Styling {
    pub(crate) icons: Option<Icons>,
    pub(crate) block_image: Option<BlockImage>,
    pub(crate) block_settings: Option<BlockSettings>,
}

impl Styling {
    getters! {
        /// Written to the plugin's main Turtle file, not `styling.ttl`.
        const icons -> Option<&Icons> = |it| it.icons.as_ref();

        /// `dgcs:blockImage`.
        const block_image -> Option<&BlockImage> = |it| it.block_image.as_ref();

        /// `dgcs:blockSettings`.
        const block_settings -> Option<&BlockSettings> = |it| it.block_settings.as_ref();
    }

    setters! {
        with_icons(icons: Icons) => icons = Some(icons);
        with_block_image(block_image: BlockImage) => block_image = Some(block_image);
        with_block_settings(settings: BlockSettings) => block_settings = Some(settings);
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use crabgram::Controls;

    use super::{BlockImage, BlockSettings, HostStyle, Parameter};
    use crate::{Frame, Sprite};

    #[derive(Controls)]
    struct Params {
        rate: f32,
        depth: f32,
        #[control(output)]
        level: f32,
    }

    #[test]
    fn a_parameter_of_a_field_and_of_its_control_are_the_same() {
        assert_eq!(
            Parameter::of(Params::RATE),
            Parameter::of(Params::CONTROLS[0])
        );
        assert_eq!(Parameter::of(Params::RATE).symbol, "rate");
    }

    #[test]
    fn a_parameter_is_found_by_its_control() {
        let settings = BlockSettings::default()
            .with_parameter(Parameter::of(Params::DEPTH).with_style(HostStyle::Fader))
            .with_parameter(Parameter::of(Params::RATE));

        assert!(settings.parameter_of(Params::DEPTH).is_some());
        assert!(settings.parameter_of(Params::RATE).is_none());
        assert!(settings.parameter_of(Params::CONTROLS[2]).is_none());
    }

    #[test]
    fn a_later_parameter_of_a_symbol_replaces_the_earlier_one_in_place() {
        let settings = BlockSettings::default()
            .with_parameter(Parameter::of(Params::RATE).with_pin(1))
            .with_parameter(Parameter::of(Params::DEPTH).with_pin(2))
            .with_parameter(Parameter::of(Params::RATE).with_pin(3));

        assert_eq!(
            settings.parameters(),
            [
                Parameter::of(Params::RATE).with_pin(3),
                Parameter::of(Params::DEPTH).with_pin(2),
            ]
        );
    }

    #[test]
    fn a_parameter_without_a_pin_or_a_look_removes_the_earlier_one() {
        let settings = BlockSettings::default()
            .with_parameter(Parameter::of(Params::RATE).with_pin(1))
            .with_parameter(Parameter::of(Params::DEPTH).with_style(HostStyle::Fader))
            .with_parameter(Parameter::of(Params::RATE));

        assert_eq!(
            settings.parameters(),
            [Parameter::of(Params::DEPTH).with_style(HostStyle::Fader)]
        );
    }

    #[test]
    fn a_block_image_draws_a_control_by_its_symbol() {
        let block = BlockImage::new("asset/block.png").with_animation(
            Params::LEVEL,
            Sprite::new(
                "asset/level.png",
                Frame::horizontal(NonZeroU32::new(40).unwrap()),
            )
            .with_position(1, 2),
        );

        assert_eq!(block.animations[0].symbol, "level");
    }
}
