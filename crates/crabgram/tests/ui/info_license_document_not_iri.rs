use crabgram::Info;

const INFO: Info = crabgram::info! {
    uri: c"urn:example:gain",
    name: "Gain",
    description: "",
    abbreviation: "GN",
    category: Utility,
    channels: 1,
    licensing: Commercial,
    license_document: "example.com/license",
};

fn main() {
    let _ = INFO;
}
