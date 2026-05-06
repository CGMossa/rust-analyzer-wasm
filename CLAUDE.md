# CLAUDE.md

Project context for AI assistants working on this repo. Disclose AI usage
in commit messages — see the parent rust-analyzer's CONTRIBUTING.md.

## What this is

A standalone browser playground that runs `rust-analyzer` in the browser
via WebAssembly. The Rust binding (`ra-wasm/`) wraps `ide::AnalysisHost`
and exposes a `WorldState` JS class via wasm-bindgen; the frontend
(`www/`) wires that into Monaco's language-service providers.

The analyzer source lives in a git submodule at `vendor/rust-analyzer`,
pinned to the `wasm` branch of [cgmossa/rust-analyzer][fork]. That
branch is upstream rust-analyzer plus a small patch making the workspace
compile for `wasm32-unknown-unknown`.

[fork]: https://github.com/cgmossa/rust-analyzer/tree/wasm

## Layout

```
ra-wasm/          wasm-bindgen crate (cdylib). Single TU; no tests.
  src/lib.rs        WorldState — methods are 1:1 wrappers of ide::Analysis
  src/to_proto.rs   ide types → Monaco/LSP-shaped JSON
  src/return_types.rs  serde structs that cross the JS boundary
rust-pack/        Tool that snapshots host's std/core/alloc → www/fake_*.rs
www/              Monaco frontend (webpack 5 + npm)
  index.js          Provider registrations
  ra-worker.js      Worker that owns the WorldState
vendor/
  rust-analyzer/  Submodule (cgmossa/rust-analyzer @ wasm)
```

## Build commands

```sh
# Generate fake_{std,core,alloc}.rs from the host toolchain
cargo run --release --manifest-path rust-pack/Cargo.toml

# Compile the wasm + bindgen
cargo build --release --manifest-path ra-wasm/Cargo.toml \
  --target wasm32-unknown-unknown
wasm-bindgen --target web \
  --out-dir ra-wasm/pkg --out-name wasm_demo \
  ra-wasm/target/wasm32-unknown-unknown/release/wasm_demo.wasm

# Front-end
cd www && npm install && npm run build      # → www/dist
cd www && npm run start                      # dev server on :8080

# Format / lint
cargo fmt --manifest-path ra-wasm/Cargo.toml
cargo fmt --manifest-path rust-pack/Cargo.toml
cargo clippy --manifest-path ra-wasm/Cargo.toml --target wasm32-unknown-unknown
```

## Architecture invariants

- `ra-wasm` is `#![cfg(target_arch = "wasm32")]` — it does not compile
  on the host. Use the wasm target for any Rust check.
- The Rust code constructs a fake single-file crate plus three sysroot
  crates (`std`, `core`, `alloc`) backed by the snapshotted source. The
  frontend re-`init`s the analyzer when content changes.
- The fake-sysroot files are loaded via `fetch()` at runtime, not
  inlined into the JS bundle. webpack emits them as content-hashed
  static assets (`asset/resource` rule in `www/webpack.config.js`).
- The WorldState worker uses a generic JS-side proxy that turns
  `state.foo(...args)` into `worker.postMessage({ which: "foo", args })`
  and dispatches via `state[which](...args)` in the worker. **Adding a
  new Rust method auto-exposes it on `state` — no JS plumbing needed.**

## Adding a new analyzer feature

1. Add `pub fn foo(...) -> JsValue` to the `impl WorldState` block in
   `ra-wasm/src/lib.rs`. Convert any `TextRange`s with
   `to_proto::text_range`, return via `serde_wasm_bindgen::to_value`.
2. If the return shape is new, add a `#[derive(Serialize)]` struct in
   `ra-wasm/src/return_types.rs`.
3. Register a Monaco provider (or editor action) in `www/index.js`
   that calls `state.foo(...)`.
4. Rebuild — see commands above. The CI workflow does the same.

## Updating the rust-analyzer pin

The submodule tracks `cgmossa/rust-analyzer` `wasm` branch. To bump:

```sh
cd vendor/rust-analyzer
git fetch origin wasm
git checkout origin/wasm
cd ../..
git add vendor/rust-analyzer
```

If the pin moves, `ide` API drift is the most common breakage. Keep
the binding in sync — most fields on `Config` structs are public, so
just compile and follow the errors.

## CI / deploy

`.github/workflows/deploy.yaml` rebuilds on every push to `main` and
publishes `www/dist/` to GitHub Pages
(<https://cgmossa.github.io/rust-analyzer-wasm/>). Action versions are
pinned to the latest majors (checkout v6, setup-node v6, etc.).
