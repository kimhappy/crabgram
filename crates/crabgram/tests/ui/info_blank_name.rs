use crabgram::Info;

const INFO: Info = crabgram::info! {
    uri: c"urn:example:blank",
    name: "  ",
    description: "",
    abbreviation: "BL",
    category: Utility,
    channels: 1,
};

fn main() {
    let _ = INFO;
}
