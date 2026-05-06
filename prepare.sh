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

# git am needs a committer identity; set one locally if missing.
if ! git config user.email > /dev/null 2>&1; then
    git config user.email "noreply@example.com"
    git config user.name  "wasm-patch"
fi

echo "Applying wasm patches to vendor/rust-analyzer ..."
git am --keep-cr "$PATCH_DIR"/0001-wasm-compile.patch
git am --keep-cr "$PATCH_DIR"/0002-wasm-paths-absolute.patch
echo "Done."
