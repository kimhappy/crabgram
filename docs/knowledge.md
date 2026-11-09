# Knowledge

## Device

- Target: Darkglass Anagram, `aarch64-unknown-linux-gnu`.
- Sample rate 48 kHz, block of 16 frames, tempo 40 to 300 BPM.
- Plugins go to `/root/.lv2`. After install, `jack2` and `lvgl-app` restart.
- A device binary may link only `libc.so.6`, `libm.so.6` and `ld-linux-aarch64.so.1`, within the glibc ceiling.
- A device build aborts on panic.

## Toolchain

- Nightly Rust, for `generic_const_exprs`.
- Builds use `-Znext-solver=coherence`.
- A crate that names `Counts::CHANNELS` or `Counts::PORTS` enables `#![feature(generic_const_exprs)]`.

## Nix

- Nix reads the project through git. Track `flake.nix` and `flake.lock`.
- For a local crabgram checkout, name it by path or `git+file:`, not `path:`, which copies `target/`.
