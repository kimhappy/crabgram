fn main() {}

fn unchecked(mut info: crabgram::Info) -> crabgram::Info {
    info.0.channels = 0;
    info
}
