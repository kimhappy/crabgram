use crabgram::Info;

const INFO: Info = crabgram::info! {
    uri: c"urn:example:tremolo",
    name: "Tremolo",
    description: "",
    abbreviation: "TREM",
    category: Utility,
    channels: 1,
};

fn main() {
    let _ = INFO;
}
