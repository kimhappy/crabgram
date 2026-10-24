fn main() {
    let _ = crabgram::Control::unchecked(
        "level",
        "Level",
        crabgram::__private::Direction::Input,
        crabgram::Kind::Toggle { default: false },
        None,
        crabgram::__private::Flags::default(),
    );
}
