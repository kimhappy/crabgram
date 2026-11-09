# Architecture

crabgram is a Cargo workspace plus a copier template. The workspace crates are in `crates/`.

## Crates

| Crate | Role |
| - | - |
| `crabgram` | The plugin API: `Plugin`, `Context`, `Info`, controls, port order, LV2 entry points (`export!`), host features, license check. The `testing` feature adds `Rig`, `Block` and `CountingAllocator`. |
| `crabgram-macros` | `#[derive(Controls)]`, `#[derive(Choice)]` and `info!`. They follow a renamed dependency. `crabgram` re-exports them. |
| `crabgram-styling` | Typed Darkglass custom styling. It writes `styling.ttl` as Turtle, lays out widgets, finds styling problems, gives image sizes and reads or writes `ui.json`. |
| `crabgram-build` | The xtask library. `run::<Project>()` gives `bundle`, `check`, `deploy` and `help`. |
| `crabgram-standalone` | The native app of every native bundle: the plugin on system audio devices under a preview of the block settings page. |

## Plugin crate (`crabgram`)

- `Plugin::INFO` holds the `Info` (URI, name, description, abbreviation, category, channels).
- `Plugin::Controls` is a struct that derives `Controls`. Each field is a typed control: `f32`, integer, `bool` or a `Choice` enum.
- Port order is fixed: audio inputs, audio outputs, enabled, reset, controls in field order, tempo.
- `runtime/lv2` builds the LV2 C entry points. `runtime/host` handles host features and change requests. `runtime/licensing` is the license check, a no-op without the `licensing` feature.
- The sample rate is 48 kHz and the block is 16 frames (`anagram.rs`). The tempo range is 40 to 300 BPM.
- `testing` runs a plugin through the real LV2 entry points. It records control states, applies change requests, and fails on heap allocation or panic in `run`.
- Features: `licensing`, `serde`, `testing`.

## Build (`crabgram-build`)

1. The xtask describes the plugin once as a `Project`.
2. `bundle` makes the LV2 bundle on a device shell and the standalone app on a native shell.
3. `check` validates styling, images and generated files. On a device build it also runs `sord_validate`, `lv2lint` and the ABI check.
4. `deploy` is device only. It runs `check`, installs the bundle over one SSH connection and restarts the `jack2` and `lvgl-app` services.

The ABI check allows only `libc.so.6`, `libm.so.6` and `ld-linux-aarch64.so.1`. It rejects a build-host runpath and the unwinder.

## Standalone (`crabgram-standalone`)

- `audio` runs devices, resamplers and ring buffers on a thread of its own. Denormals are flushed to zero while the plugin runs.
- `plugin` loads the library named by `CRABGRAM_PLUGIN` and runs it block by block.
- `render` draws the preview: the styling images over built-in widgets. `strip` is the device and settings bar. `window` uses winit and softbuffer.

## Environment

- `flake.nix` defines two shells: native (`nix develop`) and device (`nix develop .#anagram`, aarch64 cross build).
- `nix/lv2.nix` collects the LV2 extension bundles. `nix/nickel.nix` and `nix/aarch64-linux-run` support the device build.
- `template/` is the copier template (`copier.yml`) for a plugin project: a plugin crate, an `xtask` crate, optional assets, flake and README.
