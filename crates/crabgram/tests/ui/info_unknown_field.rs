use crabgram::Info;

const INFO: Info = crabgram::info! {
    uri: c"urn:example:tremolo",
    name: "Tremolo",
    description: "",
    abreviation: "TRM",
    abbreviation: "TRM",
    category: Modulator,
    channels: 1,
};

fn main() {
    let _ = INFO;
}
