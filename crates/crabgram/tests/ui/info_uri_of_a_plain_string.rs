use crabgram::Info;

const INFO: Info = crabgram::info! {
    uri: "urn:example:tremolo",
    name: "Tremolo",
    description: "",
    abbreviation: "TRM",
    category: Modulator,
    channels: 1,
};

fn main() {
    let _ = INFO;
}
