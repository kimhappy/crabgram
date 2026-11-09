use crabgram::{
    Category, Info, Licensing,
    __private::info::{Fields, checked},
};

const INFO: Info = checked(Fields {
    uri: c"urn:example:silence",
    name: "Silence",
    description: "",
    abbreviation: "SIL",
    category: Category::Utility,
    channels: 1,
    licensing: Licensing::Off,
    license_document: None,
    package: "silence",
    authors: "Someone <someone@example.com>",
    repository: "https://example.com/silence",
    license: "MIT OR LicenseRef-Silence",
    version: "0.1.0",
});

fn main() {
    let _ = INFO;
}
