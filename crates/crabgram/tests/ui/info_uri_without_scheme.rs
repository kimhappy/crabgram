use crabgram::Info;

const INFO: Info = crabgram::info! {
    uri: c"plugins/gain",
    name: "Gain",
    description: "",
    abbreviation: "GN",
    category: Utility,
    channels: 1,
};

fn main() {
    let _ = INFO;
}
