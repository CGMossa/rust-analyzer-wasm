# CLAUDE.md

Project context for AI assistants working on this repo. Disclose AI usage
in commit messages — see the parent rust-analyzer's CONTRIBUTING.md.

## What this is

A standalone browser playground that runs `rust-analyzer` in the browser
via WebAssembly. The Rust binding (`ra-wasm/`) wraps `ide::AnalysisHost`
and exposes a `WorldState` JS class via wasm-bindgen; the frontend
(`www/`) wires that into Monaco's language-service providers.

The analyzer source lives in `vendor/rust-analyzer`, a git submodule
pinned to a specific commit of upstream `rust-lang/rust-analyzer`.
Two wasm-specific patches in `patches/` are applied on top via
`just prepare` (idempotent `git am` wrapper).

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
patches/          wasm patches applied over vendor/rust-analyzer
vendor/
  rust-analyzer/  Submodule (rust-lang/rust-analyzer @ pinned commit)
```

## Just recipes

All build operations use [just](https://github.com/casey/just). Run `just --list` to see all.

| Recipe | When to reach for it |
|---|---|
| `just setup` | Once after cloning; adds `wasm32-unknown-unknown` target and `rust-src` |
| `just prepare` | After clone or after changing the submodule pin; applies `patches/` via `git am` |
| `just sysroot` | After changing the Rust toolchain; regenerates `www/fake_*.rs` |
| `just wasm` | After editing `ra-wasm/src/`; recompiles the wasm binding |
| `just deps` | After editing `www/package.json`; runs `npm ci` |
| `just web` | After editing `www/`; runs webpack production build |
| `just build` | Full rebuild from scratch: prepare + sysroot + wasm + deps + web |
| `just dev` | Start webpack dev server on :8080 (watches www/ sources) |
| `just serve` | Serve `www/dist/` locally with Python (no watch) |
| `just check` | Type-check `ra-wasm` for `wasm32-unknown-unknown` without linking |
| `just lint` | Run clippy on `ra-wasm` for `wasm32-unknown-unknown` |
| `just fmt` | Format `ra-wasm` and `rust-pack` sources |
| `just fmt-check` | Check formatting without writing (used in CI) |
| `just regen-patches` | After editing commits in `vendor/rust-analyzer`; rewrites `patches/` |
| `just bump` | Fetch upstream `master` and move submodule HEAD; run `regen-patches` after if patches conflict |
| `just wasm-size` | Show wasm binary size (requires a built wasm artifact) |
| `just audit` | Audit Cargo deps for known vulnerabilities |
| `just clean` | Remove all build artifacts (wasm target, bindgen output, npm dist) |

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

```sh
just bump              # fetch upstream master, move submodule HEAD
# if git am fails: resolve conflicts, then `git am --continue`
just regen-patches     # rewrite patches/ from the two wasm commits
git add vendor/rust-analyzer patches/
git commit -m "Bump rust-analyzer to <sha>"
```

After bumping, `ide` API drift is the most common breakage. Compile
`ra-wasm` for wasm32 (`just check`) and follow the errors.

## CI / deploy

`.github/workflows/deploy.yaml` rebuilds on every push to `main` and
publishes `www/dist/` to GitHub Pages
(<https://cgmossa.github.io/rust-analyzer-wasm/>). Action versions are
pinned to the latest majors (checkout v6, setup-node v6, etc.).
