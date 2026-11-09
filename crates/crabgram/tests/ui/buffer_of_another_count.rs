#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

use crabgram::{
    Category, Context, Controls, Info, Plugin,
    __private::info::{Fields, checked},
};

#[derive(Controls)]
struct Params {
    level: f32,
}

struct Swap;

impl Plugin for Swap {
    type Controls = Params;

    const INFO: Info = checked(Fields {
        licensing: crabgram::Licensing::Off,
        license_document: None,
        uri: c"urn:example:swap",
        name: "Swap",
        description: "",
        abbreviation: "SWP",
        category: Category::Utility,
        channels: 1,
        package: "swap",
        authors: "Someone <someone@example.com>",
        repository: "https://example.com/swap",
        license: "MIT",
        version: "0.1.0",
    });

    fn new() -> Self {
        Self
    }

    fn run(&mut self, _: &Params, context: &mut Context<'_, Self>) {
        let [left, right] = context.buffers();

        left.swap_with_slice(right);
    }
}

fn main() {}
