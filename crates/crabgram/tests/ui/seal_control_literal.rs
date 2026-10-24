fn main() {
    let _ = crabgram::Control {
        symbol: "level",
        name: "Level",
        direction: crabgram::__private::Direction::Input,
        kind: crabgram::Kind::Toggle { default: false },
        unit: None,
        flags: Default::default(),
    };
}
