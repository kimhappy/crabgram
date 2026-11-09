# Research

## Test run, 2026-11-09

Command:

```sh
nix develop -c cargo test --workspace --no-fail-fast
```

Result: every suite passes.

The shell sets `MIRIFLAGS = "-Zmiri-deterministic-floats"`, so the harness also runs under Miri. The renamed-dependency test is skipped there because it runs cargo.
