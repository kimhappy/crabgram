# Decisions

## Nix shells for every build

- Chosen: native and device shells in `flake.nix`.
- Why: the device toolchain, cross gcc and LV2 tools come pinned.
- Changes the answer: a device toolchain that needs no Nix.

## Plugin projects start from a copier template

- Chosen: `uvx copier copy gh:kimhappy/crabgram`.
- Why: the project needs a nightly toolchain, an xtask and a flake that match the crates.
- Template and crates stay in step (see `CLAUDE.md`).

## Heap-free, panic-free `run`

- Chosen: the test harness fails a `run` that allocates or panics.
- Why: `run` is real-time on the device. A device build aborts on panic and the ABI check rejects the unwinder.

## Styling is typed and checked at build time

- Chosen: Rust types write `styling.ttl`. `check` finds mistakes without building the device image.
- Why: a wrong Turtle file would only show on the device.

## Default deploy host `root@192.168.51.1`

- Chosen: the fixed address of the device (`DEVICE_HOST`). `--host` overrides it.
- Rejected: the host name `anagram`.

## Standalone app for native bundles

- Chosen: a native app that loads the same plugin library and draws the same screen.
- Why: the plugin can be tried without the device.

## Licensing

- GPLv3, or a purchased license on request.
