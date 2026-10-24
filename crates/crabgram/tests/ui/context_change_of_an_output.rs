#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

use crabgram::{
    Category, Context, Controls, Info, Plugin,
    __private::info::{Fields, checked},
};

#[derive(Controls)]
struct Params {
    level: f32,
    #[control(output)]
    peak: f32,
}

struct Asker;

impl Plugin for Asker {
    type Controls = Params;

    const INFO: Info = checked(Fields {
        licensing: crabgram::Licensing::Off,
        license_document: None,
        uri: c"urn:example:asker",
        name: "Asker",
        description: "",
        abbreviation: "ASK",
        category: Category::Utility,
        channels: 1,
        package: "asker",
        authors: "Someone <someone@example.com>",
        repository: "https://example.com/asker",
        license: "MIT",
        version: "0.1.0",
    });

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _: &Params, context: &mut Context<'_, Self>) {
        context.request_change(Params::PEAK, 0.5);
    }
}

fn main() {}
