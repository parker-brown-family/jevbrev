#!/usr/bin/env bash
# Build the npm package in npm/ from the Rust crate, then check it prints what the crate prints.
# Needs: rustup target wasm32-unknown-unknown, and wasm-bindgen-cli at the version pinned in Cargo.toml.
set -euo pipefail
cd "$(dirname "$0")/.."

want=$(sed -n 's/^wasm-bindgen = { version = "=\([0-9.]*\)".*/\1/p' Cargo.toml)
have=$(wasm-bindgen --version | awk '{print $2}')
if [[ "$want" != "$have" ]]; then
  echo "wasm-bindgen-cli is $have; Cargo.toml pins $want. Run: cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

cargo build --lib --release --target wasm32-unknown-unknown --features wasm
wasm-bindgen --target nodejs --out-dir npm --out-name jevbrev \
  --remove-name-section --remove-producers-section \
  target/wasm32-unknown-unknown/release/jevbrev.wasm
cp README.md LICENSE-MIT LICENSE-APACHE npm/

crate=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
pkg=$(node -p 'require("./npm/package.json").version')
if [[ "$crate" != "$pkg" ]]; then
  echo "Cargo.toml is $crate but npm/package.json is $pkg" >&2
  exit 1
fi

node --test tests/node/
echo "npm/ is ready: $(du -h npm/jevbrev_bg.wasm | cut -f1) of WebAssembly"
