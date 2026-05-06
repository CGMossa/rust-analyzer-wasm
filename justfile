# rust-analyzer-wasm justfile
# Requires: just, cargo (stable + wasm32-unknown-unknown + rust-src), wasm-bindgen-cli, node

set shell := ["bash", "-euo", "pipefail", "-c"]

# Print available recipes
default:
    @just --list

# ── Setup ────────────────────────────────────────────────────────────────────

# Install Rust targets and components required for this project
setup:
    rustup target add wasm32-unknown-unknown
    rustup component add rust-src

# Apply wasm patches to vendor/rust-analyzer (idempotent)
prepare:
    ./prepare.sh

# ── Build steps ──────────────────────────────────────────────────────────────

# Build fake_{std,core,alloc}.rs from the host sysroot into www/
sysroot:
    cargo run --release --manifest-path rust-pack/Cargo.toml

# Compile ra-wasm to wasm32-unknown-unknown and run wasm-bindgen
wasm:
    cargo build --release --manifest-path ra-wasm/Cargo.toml \
        --target wasm32-unknown-unknown
    wasm-bindgen --target web \
        --out-dir ra-wasm/pkg --out-name wasm_demo \
        ra-wasm/target/wasm32-unknown-unknown/release/wasm_demo.wasm

# Shrink the wasm-bindgen output with binaryen's wasm-opt (~30% reduction).
# Skipped silently if wasm-opt isn't installed — keeps local dev cheap.
wasm-opt:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v wasm-opt >/dev/null 2>&1; then
        echo "wasm-opt not found, skipping (install binaryen for production builds)"
        exit 0
    fi
    SRC=ra-wasm/pkg/wasm_demo_bg.wasm
    BEFORE=$(wc -c <"$SRC")
    wasm-opt -Oz -all "$SRC" -o "$SRC.opt"
    mv "$SRC.opt" "$SRC"
    AFTER=$(wc -c <"$SRC")
    awk -v b="$BEFORE" -v a="$AFTER" 'BEGIN { printf "wasm-opt: %.1f MB → %.1f MB (-%.0f%%)\n", b/1048576, a/1048576, (1-a/b)*100 }'

# Install npm dependencies for the www/ frontend
deps:
    npm --prefix www ci

# Bundle the Monaco frontend (production build → www/dist)
web:
    npm --prefix www run build

# Full production build: prepare → sysroot → wasm → wasm-opt → deps → web
build: prepare sysroot wasm wasm-opt deps web

# ── Development ──────────────────────────────────────────────────────────────

# Start the webpack dev server on localhost:8080 (watches www/ sources)
dev: prepare sysroot wasm
    npm --prefix www run start

# Serve www/dist/ on a local HTTP server (requires `python3`)
serve:
    python3 -m http.server -d www/dist 8080

# ── Quality ──────────────────────────────────────────────────────────────────

# Type-check ra-wasm for wasm32 (no link, just rustc)
check:
    cargo check --manifest-path ra-wasm/Cargo.toml \
        --target wasm32-unknown-unknown

# Run clippy on ra-wasm for wasm32
lint:
    cargo clippy --manifest-path ra-wasm/Cargo.toml \
        --target wasm32-unknown-unknown

# Format ra-wasm and rust-pack sources
fmt:
    cargo fmt --manifest-path ra-wasm/Cargo.toml
    cargo fmt --manifest-path rust-pack/Cargo.toml

# Check formatting without writing changes
fmt-check:
    cargo fmt --manifest-path ra-wasm/Cargo.toml -- --check
    cargo fmt --manifest-path rust-pack/Cargo.toml -- --check

# ── Maintenance ───────────────────────────────────────────────────────────────

# Regenerate patches/ from the current vendor/rust-analyzer HEAD
# Expects exactly 2 wasm commits on top of the upstream base.
# Run after manually editing vendor/rust-analyzer commits.
regen-patches:
    #!/usr/bin/env bash
    set -euo pipefail
    # Find the upstream base: last commit whose message does NOT contain "wasm"
    BASE=$(git -C vendor/rust-analyzer log --oneline | \
        grep -iv "wasm" | head -1 | cut -d' ' -f1)
    echo "Base commit: $BASE"
    COUNT=$(git -C vendor/rust-analyzer rev-list "$BASE"..HEAD --count)
    if [ "$COUNT" -ne 2 ]; then
        echo "Expected 2 wasm commits on top of base, found $COUNT. Aborting."
        exit 1
    fi
    rm -f patches/*.patch
    git -C vendor/rust-analyzer format-patch -1 HEAD~1 -o "$(pwd)/patches/"
    git -C vendor/rust-analyzer format-patch -1 HEAD   -o "$(pwd)/patches/"
    # Rename to stable names
    FIRST=$(ls patches/0001-*.patch 2>/dev/null | head -1)
    SECOND=$(ls patches/0002-*.patch 2>/dev/null | head -1)
    [ -n "$FIRST"  ] && mv "$FIRST"  patches/0001-wasm-compile.patch
    [ -n "$SECOND" ] && mv "$SECOND" patches/0002-wasm-paths-absolute.patch
    echo "Wrote patches/0001-wasm-compile.patch and patches/0002-wasm-paths-absolute.patch"

# Bump vendor/rust-analyzer to the latest upstream master
# After bumping, run `just regen-patches` if the patches need rebasing
bump:
    cd vendor/rust-analyzer && git fetch origin master && git checkout origin/master
    @echo "Submodule bumped. If patches conflict, resolve with git am --continue then run: just regen-patches"

# Show the wasm binary size (requires wasm to be built)
wasm-size:
    wc -c ra-wasm/target/wasm32-unknown-unknown/release/wasm_demo.wasm | \
        awk '{printf "wasm binary: %.1f MB (%d bytes)\n", $1/1024/1024, $1}'
    wc -c ra-wasm/pkg/wasm_demo_bg.wasm | \
        awk '{printf "wasm-bindgen output: %.1f MB (%d bytes)\n", $1/1024/1024, $1}'

# Audit Cargo dependencies for known vulnerabilities
audit:
    cargo audit --manifest-path ra-wasm/Cargo.toml

# Headless smoke test: serves www/dist on :8090 and runs the completions
# probe against it. Requires www/dist already built and python3.
smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -f www/dist/index.html ]; then
        echo "www/dist/index.html not found — run 'just web' first." >&2
        exit 1
    fi
    npm --prefix tests/headless ci
    python3 -m http.server -d www/dist 8090 >/tmp/smoke-http.log 2>&1 &
    SERVER_PID=$!
    trap "kill $SERVER_PID 2>/dev/null || true" EXIT
    for _ in $(seq 1 50); do
        curl -fs http://127.0.0.1:8090/ >/dev/null && break
        sleep 0.2
    done
    node tests/headless/probe-completions.js http://127.0.0.1:8090/

# Update tests/headless/node_modules with the lockfile.
smoke-deps:
    npm --prefix tests/headless ci

# ── Clean ─────────────────────────────────────────────────────────────────────

# Remove all build artifacts (wasm target, bindgen output, npm dist)
clean:
    rm -rf ra-wasm/target ra-wasm/pkg
    rm -rf rust-pack/target
    rm -rf www/dist www/node_modules
