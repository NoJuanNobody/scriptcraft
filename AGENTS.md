# ScriptCraft — instructions for agents

ScriptCraft is a clean-room, open-source, Rust-native screenwriting app targeting Final Draft 13
workflow parity. Conventions follow the sibling Craft apps (WordCraft et al.).

## Never crash
- No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` in non-test code;
  no `unsafe` (`unsafe_code = "forbid"`). Crate roots carry
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`.
- Input-derived numbers are hostile (files, command params, MCP): `get()` not `[i]`, slice strings
  only at char boundaries, clamp, cap allocations.
- `Session::run` restores the script if a command errors or panics — a safety net, not a licence.
- `crates/engine/src/tests.rs::hostile_params_never_panic` runs every command with junk; keep it green.

## Clean room and assets
- Never copy Final Draft icons, images, templates, sample scripts, fonts or wording (feature names only).
- Every asset (font, image, sample script) has a row in ATTRIBUTION.md; `cargo xtask assets` checks.
- Icons are drawn in code (`crates/ui-egui/src/icons.rs`). Typeface: Courier Prime (OFL).
- File formats come from public structure: Fountain (fountain.io) and observed FDX XML. Never read GPL code.

## Architecture
See README › Architecture. Everything is a command (`CommandSpec` in `crates/engine/src/cmd/*.rs`;
UI-only `ui.*` commands in `crates/ui-egui/src/lib.rs`). Nothing below `ui-egui` depends on egui
(`cargo xtask layers`). Parity catalog: `crates/engine/src/catalog.rs` → `cargo xtask parity`
writes `docs/parity-checklist.md`.

## Typing behaves like Final Draft
`docs/typing-parity.md`, pinned by `crates/engine/src/tests_typing.rs`. Change behaviour, doc and tests together.

## Quality gate
Before every commit: `cargo xtask ci` (fmt check, clippy `-D warnings`, tests, assets, layers,
parity doc fresh). Use a `CARGO_TARGET_DIR` inside the repo (`.target/`, gitignored).

## Running and looking at the app
- `cargo run --release -p scriptcraft -- --control 7990` (sample script + control channel; key in
  `~/Library/Application Support/ScriptCraft/control-key.local-7990`). See `docs/control-protocol.md`.
- Headless screenshots: `cargo run -p scriptcraft-ui-egui --example ui_shot -- script.jsonl`. For UI
  work, look at the PNG.
- CLI: `scriptcraft-cli convert|commands|run|report|parity|mcp`. MCP: `docs/mcp.md`.
- Settings use `craft-settings` (storytold/craft-libs). Run the app with `SCRIPTCRAFT_CONFIG_DIR=`
  (empty: isolated) or `SCRIPTCRAFT_CONFIG_DIR=/tmp/x` when testing so a real profile is never
  touched; `scriptcraft_control_key::folders` is the one place folder rules are decided.
- Browser build: `cargo xtask wasm` (compile check, part of `ci` when the wasm32 target is
  installed) and `cargo xtask web` (needs `wasm-bindgen-cli` at the version in Cargo.lock) →
  `apps/scriptcraft-web/dist/`; serve it with any static server. Keep everything below the web
  app free of native-only APIs (no `std::fs` on the browser path; file I/O goes through
  `file.load` / `document.export` and `ScriptApp::host`).
