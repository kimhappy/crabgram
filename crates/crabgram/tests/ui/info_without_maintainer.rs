use crabgram::{
    Category, Info, Licensing,
    __private::info::{Fields, checked},
};

const INFO: Info = checked(Fields {
    uri: c"urn:example:tremolo",
    name: "Tremolo",
    description: "",
    abbreviation: "TRM",
    category: Category::Modulator,
    channels: 1,
    licensing: Licensing::Off,
    license_document: None,
    package: "tremolo",
    authors: "",
    repository: "https://example.com/tremolo",
    license: "MIT",
    version: "0.1.0",
});

fn main() {
    let _ = INFO;
}
