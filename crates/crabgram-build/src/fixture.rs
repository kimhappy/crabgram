use core::marker::PhantomData;

use crabgram::{
    __private::info::{Fields, checked},
    Category, Context, Controls, Info, Licensing, Plugin,
};

use crate::{Assets, Project};

#[derive(Controls)]
pub struct Params {
    pub gain: f32,
}

#[derive(Controls)]
pub struct Blockable {
    #[control(blockable)]
    pub gain: f32,
}

/// A mono gain plugin with controls `C`, commercially licensed when `LICENSED`.
pub struct Gain<C = Params, const LICENSED: bool = false>(PhantomData<fn() -> C>);

pub type Licensed = Gain<Params, true>;

pub type Blocking = Gain<Blockable>;

impl<C: Controls, const LICENSED: bool> Plugin for Gain<C, LICENSED> {
    type Controls = C;

    const INFO: Info = checked(Fields {
        uri: c"urn:example:gain",
        name: "Gain",
        description: "",
        abbreviation: "GN",
        category: Category::Utility,
        channels: 1,
        licensing: if LICENSED {
            Licensing::Commercial
        } else {
            Licensing::Off
        },
        license_document: None,
        package: env!("CARGO_PKG_NAME"),
        authors: env!("CARGO_PKG_AUTHORS"),
        repository: env!("CARGO_PKG_REPOSITORY"),
        license: env!("CARGO_PKG_LICENSE"),
        version: env!("CARGO_PKG_VERSION"),
    });

    fn new() -> Self {
        Self(PhantomData)
    }

    fn run(&mut self, _: &C, _: &mut Context<'_, Self>) {}
}

/// The packages of [`Documented`]: its own, then two the bundle needs for itself.
const PACKAGES: [&str; 3] = ["gain", "manifest", crate::ASSET_DIRECTORY];

/// A gain plugin that names its authors, repository and licenses, packaged as
/// `PACKAGES[PACKAGE]`.
pub struct Documented<const PACKAGE: usize = 0>;

impl<const PACKAGE: usize> Plugin for Documented<PACKAGE> {
    type Controls = Params;

    const INFO: Info = checked(Fields {
        uri: c"urn:example:gain",
        name: "Gain",
        description: "",
        abbreviation: "GN",
        category: Category::Utility,
        channels: 1,
        package: PACKAGES[PACKAGE],
        authors: "Someone <someone@example.com>",
        repository: "https://example.com/gain",
        license: "MIT OR LicenseRef-Gain",
        version: "0.1.0",
        licensing: Licensing::Off,
        license_document: Some("https://example.com/gain/license"),
    });

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _: &Params, _: &mut Context<'_, Self>) {}
}

pub struct Xtask;

/// A project packaged as a file or directory of the bundle, `PACKAGES[PACKAGE]`.
pub struct Reserved<const PACKAGE: usize>;

impl<const PACKAGE: usize> Project for Reserved<PACKAGE> {
    type Plugin = Documented<PACKAGE>;
}

impl Project for Xtask {
    type Plugin = Gain;
}

/// A project with an asset directory of `target/` for the tests to fill.
pub struct Knobbed;

impl Knobbed {
    pub const DIRECTORY: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/crabgram-build-knobbed"
    );
}

impl Project for Knobbed {
    type Plugin = Gain;

    const ASSETS: Option<Assets> = Some(Assets::at_absolute(Self::DIRECTORY));
}

/// A project whose styling names `asset/knob.png` in a directory of `target/` the tests write.
pub struct Styled;

impl Styled {
    pub const KNOB: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/crabgram-build-fixture/knob.png"
    );
}

impl Project for Styled {
    type Plugin = Gain;

    const ASSETS: Option<Assets> = Some(Assets::at_absolute(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/crabgram-build-fixture"
    )));

    fn styling() -> crabgram_styling::Styling {
        crabgram_styling::Styling::default().with_block_settings(
            crabgram_styling::BlockSettings::default()
                .with_knob(crabgram_styling::Widget::new("asset/knob.png")),
        )
    }
}
