# Contributing to ScriptCraft

Thanks for your interest. Issues and pull requests are welcome, from bug reports to new features.
ScriptCraft is a clean-room, open-source screenwriting app in pure Rust, so a few rules keep it
safe and legally clean.

## Before you start

- **Small fixes:** open a pull request directly.
- **Larger changes:** open an issue first so we can agree on the approach. The
  [ROADMAP](ROADMAP.md) and the [parity checklist](docs/parity-checklist.md) list what is planned.
- **Security problems:** do not use a public issue. See [SECURITY.md](SECURITY.md).

## Setup

You need Rust 1.95 or newer (edition 2024).

```sh
git clone https://github.com/NoJuanNobody/scriptcraft && cd scriptcraft
cargo run --release -p scriptcraft          # opens the sample script
cargo xtask ci                              # the full quality gate
```

Use a `CARGO_TARGET_DIR` inside the repo (`.target/`, gitignored). When testing, run the app with
`SCRIPTCRAFT_CONFIG_DIR=` (set but empty) so your real settings are never touched.

## The rules

These are explained in [AGENTS.md](AGENTS.md); in short:

1. **Never crash.** No `unwrap`, `expect`, `panic!`, `todo!`, `unreachable!` or `unsafe` in non-test
   code. Treat file contents, command parameters and MCP input as hostile: use `get()` not `[i]`,
   slice strings at character boundaries, clamp numbers and cap allocations.
2. **Everything is a command.** New behavior is a `CommandSpec` in `crates/engine/src/cmd/*.rs`, so
   menus, shortcuts, the CLI, the control channel and MCP all get it.
3. **Respect the layers.** Lower crates never depend on higher ones, and nothing below the UI
   depends on egui. `cargo xtask layers` checks it.
4. **Clean room.** Never copy Final Draft icons, images, templates, sample scripts, fonts or
   wording, and never read GPL code. File formats come from public structure (Fountain at fountain.io
   and observed FDX XML).
5. **Attribute every asset.** Each font, image or sample script needs a row in
   [ATTRIBUTION.md](ATTRIBUTION.md). `cargo xtask assets` checks it.
6. **Change behavior, docs and tests together.** Typing behavior is pinned by
   [docs/typing-parity.md](docs/typing-parity.md) and `crates/engine/src/tests_typing.rs`.

## Before you open a pull request

Run `cargo xtask ci`. It checks formatting, `clippy -D warnings`, tests, assets, layers and that the
parity doc is up to date. Pull requests need it to pass.

For a change you can see, include a screenshot from `ui_shot` or a short recording (see
`docs/demo/`), and look at the result yourself first.

## Licensing

ScriptCraft is `MIT OR Apache-2.0`. By submitting a contribution you agree that it is licensed
under the same terms, without extra conditions.

## Conduct

Be respectful and assume good intent. Harassment or personal attacks are not welcome. See
[GOVERNANCE.md](GOVERNANCE.md) for how disagreements and moderation are handled.
