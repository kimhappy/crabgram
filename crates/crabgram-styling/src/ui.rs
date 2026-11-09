//! The `ui.json` of a plugin: its controls and styling, without linking the plugin.

use std::{io, num::NonZeroU32};

use crabgram::{Control, port::Layout};
use serde::{Deserialize, Serialize};

use crate::{BlockSettings, model::getters};

/// What a UI needs to draw and drive a block settings screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    uri: String,
    name: String,
    channels: NonZeroU32,
    controls: Vec<Control>,
    settings: BlockSettings,
}

/// The file the standalone app reads the [`Model`] from.
pub const FILE: &str = "ui.json";

impl Model {
    /// The model of plugin `uri` named `name`, with `channels` audio channels and `controls` in port
    /// order, drawn by `settings`.
    #[must_use]
    pub fn new(
        uri: impl Into<String>,
        name: impl Into<String>,
        channels: NonZeroU32,
        controls: Vec<Control>,
        settings: BlockSettings,
    ) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            channels,
            controls,
            settings,
        }
    }

    getters! {
        /// The plugin URI, which names the library the model belongs to.
        const uri -> &str = |it| it.uri.as_str();

        /// The plugin name, [`Info::name`](crabgram::Info::name).
        const name -> &str = |it| it.name.as_str();

        /// Audio channels.
        const channels -> NonZeroU32 = |it| it.channels;

        /// In port order.
        const controls -> &[Control] = |it| it.controls.as_slice();

        /// The default gives the stock look.
        const settings -> &BlockSettings = |it| &it.settings;
    }

    /// The channel and control counts, which fix every port index.
    #[must_use]
    pub const fn layout(&self) -> Layout {
        Layout::new(self.channels.get(), self.controls.len() as u32)
    }

    /// Writes `ui.json` to `writer`.
    ///
    /// # Errors
    ///
    /// When `writer` fails, or a control cannot be written as JSON.
    pub fn write(&self, writer: impl io::Write) -> serde_json::Result<()> {
        serde_json::to_writer_pretty(writer, self)
    }

    /// Reads `ui.json` from its bytes, `json`, leaking the strings of its controls.
    ///
    /// # Errors
    ///
    /// When `json` is malformed, has no channels, or a control holds values no port takes.
    pub fn read(json: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(json)
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use crabgram::{Controls, port::Layout};

    use super::Model;
    use crate::BlockSettings;

    #[derive(Controls)]
    struct Params {
        #[control(default = 2.0, min = 1.0, max = 4.0)]
        level: f32,
    }

    fn written() -> serde_json::Value {
        let model = Model::new(
            "urn:crabgram:test",
            "Test",
            NonZeroU32::MIN,
            vec![*Params::LEVEL.as_ref()],
            BlockSettings::default(),
        );
        let mut json = Vec::new();

        model.write(&mut json).unwrap();
        serde_json::from_slice(&json).unwrap()
    }

    #[test]
    fn a_written_model_reads_back() {
        let model = Model::read(written().to_string().as_bytes()).unwrap();

        assert_eq!(model.controls(), [*Params::LEVEL.as_ref()]);
        assert_eq!(model.uri(), "urn:crabgram:test");
    }

    #[test]
    fn the_layout_follows_the_channels_and_the_controls() {
        let model = Model::read(written().to_string().as_bytes()).unwrap();

        assert_eq!(model.layout(), Layout::new(1, 1));
    }

    #[test]
    fn the_json_keeps_its_keys() {
        let json = written();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();

        keys.sort_unstable();

        assert_eq!(keys, ["channels", "controls", "name", "settings", "uri"]);
        assert_eq!(json["controls"][0]["kind"]["Float"]["max"], 4.0);
    }

    #[test]
    fn the_settings_read_back_as_written() {
        let model = Model::new(
            "urn:crabgram:test",
            "Test",
            NonZeroU32::MIN,
            vec![*Params::LEVEL.as_ref()],
            BlockSettings::default()
                .with_parameter(crate::Parameter::of(Params::LEVEL).with_pin(3)),
        );
        let mut json = Vec::new();

        model.write(&mut json).unwrap();

        assert_eq!(Model::read(&json).unwrap().settings(), model.settings());
    }

    #[test]
    fn a_model_without_a_uri_fails_to_read() {
        let mut json = written();

        json.as_object_mut().unwrap().remove("uri");

        assert!(Model::read(json.to_string().as_bytes()).is_err());
    }

    #[test]
    fn a_model_without_channels_fails_to_read() {
        let mut json = written();

        json["channels"] = 0.into();

        assert!(Model::read(json.to_string().as_bytes()).is_err());
    }

    #[test]
    fn a_minimum_above_the_maximum_fails_to_read() {
        let mut json = written();

        json["controls"][0]["kind"]["Float"]["min"] = 8.0.into();

        let error = Model::read(json.to_string().as_bytes()).unwrap_err();

        assert!(error.to_string().contains("level"), "{error}");
    }

    #[test]
    fn an_expensive_output_fails_to_read() {
        let mut json = written();

        json["controls"][0]["direction"] = "Output".into();
        json["controls"][0]["expensive"] = true.into();

        let error = Model::read(json.to_string().as_bytes()).unwrap_err();

        assert!(error.to_string().contains("level"), "{error}");
    }
}
