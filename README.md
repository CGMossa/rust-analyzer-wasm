# rust-analyzer-wasm

rust-analyzer running in the browser via WebAssembly.
Live site: **https://cgmossa.github.io/rust-analyzer-wasm/**

The Rust binding (`ra-wasm/`) wraps `ide::AnalysisHost` with wasm-bindgen.
The frontend (`www/`) wires Monaco editor to the wasm API. `rust-pack/`
snapshots `std`/`core`/`alloc` source from the host toolchain so the
analyzer has a fake sysroot to work with.

`vendor/rust-analyzer` is a git submodule pinned to a specific upstream
commit. `patches/` holds the two wasm-specific changes applied on top;
`prepare.sh` (and `just prepare`) applies them idempotently.

## Layout

```
ra-wasm/          wasm-bindgen crate (cdylib), exports WorldState
rust-pack/        builds fake_{std,core,alloc}.rs from the host sysroot
www/              Monaco frontend (webpack 5)
patches/          wasm patches applied over vendor/rust-analyzer
vendor/
  rust-analyzer/  submodule: rust-lang/rust-analyzer @ pinned commit
```

## Build

Prereqs: stable Rust, `wasm32-unknown-unknown` target, `wasm-bindgen-cli`,
Node 22+, and [just](https://github.com/casey/just).

```sh
git clone --recurse-submodules https://github.com/cgmossa/rust-analyzer-wasm.git
cd rust-analyzer-wasm
just setup    # add wasm target + rust-src component
just build    # prepare patches, sysroot, wasm, npm bundle -> www/dist
just dev      # dev server on localhost:8080
```

See `just --list` for all recipes.

## Updating the rust-analyzer pin

```sh
just bump              # fetch upstream master, move submodule HEAD
# resolve any patch conflicts in vendor/rust-analyzer, then:
just regen-patches     # rewrite patches/ from the new commits
git add vendor/rust-analyzer patches/
git commit -m "Bump rust-analyzer to <sha>"
```

## CI

`.github/workflows/deploy.yaml` runs on every push to `main` and
publishes `www/dist/` to the `gh-pages` branch.
