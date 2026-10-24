# crabgram

Write LV2 plugins for the Darkglass Anagram in Rust.

## Features

- Builds the LV2 bundle for the device, and a native standalone app of the same plugin and screen
- Custom styling checked at build time
  - Block images, animated sprites, backgrounds and overlays
  - Knob, list, toggle and meter widgets, styled per kind or per control
  - Slot pinning, padding and multi-slot lists
- Typed parameter controls
  - `f32` (optionally logarithmic), integer, `bool` and `Choice` enum
  - Range, default, name and unit
  - Quick pot, hidden, saved, expensive and blockable flags
  - Output controls for meters and animated block images
  - Set an input inactive or blocked, or request a new value for it, checked by type
  - Read the tempo in BPM
- Test harness
  - Runs the plugin through its real LV2 entry points
  - Records control states and applies change requests
  - Fails on heap allocation or panic in `run`, and runs under Miri
- `bundle`, `check`, `deploy` and `help` commands; `deploy` installs over one SSH connection
  - `check` on a device build also runs `sord_validate`, `lv2lint` and an ABI check
- Optional license check, `Info::licensing`

## Prerequisites

- [Nix](https://nixos.org/download)

## Start from a template

```sh
uvx copier copy gh:kimhappy/crabgram my_plugin
cd my_plugin && git init && git add -N flake.nix
```

Nix reads the project through git, so `flake.nix` and `flake.lock` must be tracked: run `git add flake.lock` after the first `nix develop`.

## Example

```rust
#![feature(generic_const_exprs)]

use crabgram::prelude::*;

#[derive(Controls)]
pub struct Params {
    #[control(default = 0.0, min = -24.0, max = 12.0, unit = Decibel, quick_pot)]
    pub gain: f32,
}

pub struct Gain {
    gain: f32,
}

impl Plugin for Gain {
    type Controls = Params;

    const INFO: Info = info! {
        uri: c"https://example.com/plugins/gain",
        name: "Gain",
        description: "Turns the level up or down.",
        abbreviation: "GN",
        category: Utility,
        channels: 1,
    };

    fn new() -> Self {
        Self { gain: 1.0 }
    }

    fn run(&mut self, controls: &Params, context: &mut Context<'_, Self>) {
        let target = if context.is_enabled() {
            10.0_f32.powf(controls.gain / 20.0)
        } else {
            1.0
        };
        let start = if context.is_reset() { target } else { self.gain };
        let step = (target - start) / FRAMES as f32;
        let [samples] = context.buffers();

        samples
            .iter_mut()
            .zip(1..)
            .for_each(|(sample, frame)| *sample *= start + step * frame as f32);
        self.gain = target;
    }
}

export!(Gain);
```

## License

GPLv3 or contact babtul21@gmail.com to purchase a license.

Fonts: the standalone app embeds Roboto Mono (Apache 2.0) and Barlow Condensed (OFL).
