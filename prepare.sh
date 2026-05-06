#!/usr/bin/env bash
# Apply wasm patches to vendor/rust-analyzer (idempotent).
# Run this after cloning or after changing the submodule pin.
set -euo pipefail

PATCH_DIR="$(cd "$(dirname "$0")/patches" && pwd)"
RA_DIR="$(cd "$(dirname "$0")/vendor/rust-analyzer" && pwd)"

cd "$RA_DIR"

# Marker: the wasm_file_url shim is created by patch 0001.
if [ -f "crates/rust-analyzer/src/lsp/wasm_file_url.rs" ]; then
    echo "Patches already applied, skipping."
    exit 0
fi

# Pass identity inline so it works regardless of repo/global config state.
echo "Applying wasm patches to vendor/rust-analyzer ..."
GIT_COMMITTER_NAME="wasm-patch"   GIT_AUTHOR_NAME="wasm-patch"   \
GIT_COMMITTER_EMAIL="patch@local" GIT_AUTHOR_EMAIL="patch@local" \
    git -c user.name="wasm-patch" -c user.email="patch@local" \
        am --keep-cr "$PATCH_DIR"/*.patch
echo "Done."
