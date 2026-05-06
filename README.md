# rust-analyzer-wasm

Browser playground for [rust-analyzer](https://github.com/rust-lang/rust-analyzer)
compiled to WebAssembly. Live at:
**https://cgmossa.github.io/rust-analyzer-wasm/**

This is a revival of the original [rust-analyzer/rust-analyzer-wasm][orig]
demo, ported against the current `rust-analyzer` source. Wires Monaco up
to a wasm build of the analyzer with completions, hover, inlay hints,
diagnostics, code actions (assists/quickfixes), goto-def/-impl/-decl,
rename, find-references, signature help, document/folding/symbol providers,
plus `expand_macro` / `view_hir` / `view_mir` / `view_syntax_tree` for
introspection.

[orig]: https://github.com/rust-analyzer/rust-analyzer-wasm

## Layout

```
ra-wasm/          Rust binding crate (wasm-bindgen) → exports WorldState
rust-pack/        Builds fake_{std,core,alloc}.rs from the host sysroot
www/              Monaco editor frontend
vendor/
  rust-analyzer/  Submodule: cgmossa/rust-analyzer @ wasm branch
                  (the patches that make rust-analyzer compile to
                   wasm32-unknown-unknown live there)
```

## Build

Prereqs: stable Rust with the `wasm32-unknown-unknown` target,
[`wasm-bindgen-cli`](https://crates.io/crates/wasm-bindgen-cli),
and Node 20+.

```sh
git clone --recurse-submodules https://github.com/cgmossa/rust-analyzer-wasm.git
cd rust-analyzer-wasm

# 1. Build the fake sysroot from the host toolchain (writes www/fake_*.rs)
cargo run --release --manifest-path rust-pack/Cargo.toml

# 2. Build the wasm binding
cargo build --release --manifest-path ra-wasm/Cargo.toml \
  --target wasm32-unknown-unknown
wasm-bindgen --target web \
  --out-dir ra-wasm/pkg --out-name wasm_demo \
  ra-wasm/target/wasm32-unknown-unknown/release/wasm_demo.wasm

# 3. Build the web bundle
cd www
npm install
npm run build      # production → www/dist
npm run start      # dev server on localhost:8080
```

## Continuous deploy

`.github/workflows/deploy.yaml` rebuilds steps 1–3 on every push to `main`
and publishes `www/dist/` to GitHub Pages.

## Updating the rust-analyzer pin

```sh
cd vendor/rust-analyzer
git pull origin wasm
cd ../..
git add vendor/rust-analyzer
git commit -m "Bump rust-analyzer submodule"
```

The submodule tracks the [`wasm` branch on cgmossa/rust-analyzer][fork].
That branch carries two patches on top of upstream:

- gate the `home` crate off for `wasm32-unknown-unknown` in
  `crates/toolchain`
- a `WasmFileUrlExt` shim because `url::Url::{from,to}_file_path` isn't
  available on `wasm32-unknown-unknown`

[fork]: https://github.com/cgmossa/rust-analyzer/tree/wasm

## Credits

- Original demo: <https://github.com/rust-analyzer/rust-analyzer-wasm>
- rust-analyzer: <https://github.com/rust-lang/rust-analyzer>
