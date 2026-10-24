use crabgram::Info;

const INFO: Info = crabgram::info! {
    uri: c"urn:example:silence",
    name: "Silence",
    description: "",
    abbreviation: "SIL",
    category: Utility,
    channels: 0,
};

fn main() {
    let _ = INFO;
}
