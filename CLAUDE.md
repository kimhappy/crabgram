# crabgram

Session context. Detail is in `docs/`.

## Keep true

- Framework for LV2 plugins on the Darkglass Anagram. Workspace crates are in `crates/`; the copier template is in `template/`.
- Enter `nix develop` for native work and `nix develop .#anagram` for the device build.
- Test with `cargo test --workspace`. Format with `cargo fmt` (`rustfmt.toml`).
- `Plugin::run` is real-time: no heap allocation, no panic. The test harness fails on both.
- Controls come from `#[derive(Controls)]`. The `expensive` flag stays supported.
- The default deploy host is `root@192.168.51.1`. It is set in `crates/crabgram-build/src/deploy.rs` (`DEVICE_HOST`).
- Generated projects use the same host. Keep `template/` in step with the crates.

## Comments and documents

Short sentences. One fact per sentence. Everything is in English.

- **Comment.** Explanation of the code itself. Always keep it to a minimum.
- **`README.md`.** Overview and how to use it.
- **`docs/architecture.md`.** How the project is organized and how it works.
- **`docs/decisions.md`.** What was chosen, why, what was rejected, and what would change the answer.
- **`docs/issues.md`.** When, what broke, why, how it was fixed.
- **`docs/research.md`.** Experiments, the commands, and the results.
- **`docs/knowledge.md`.** Background, and results distilled from research.
- **`docs/todo.md`.** Deferred work, and why it is not being done now.
- **`docs/notes.md`.** Anything else.
- **`CLAUDE.md`.** Only what every session must have. Move a write-up to `docs/`.

No comment or document names a commit id. Describe what that implementation did instead.
