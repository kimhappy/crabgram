use crabgram::Info;

const INFO: Info = crabgram::info! {
    uri: c"urn:example:tremolo",
    name: "Tremolo",
    description: "",
    category: Modulator,
};

fn main() {
    let _ = INFO;
}
