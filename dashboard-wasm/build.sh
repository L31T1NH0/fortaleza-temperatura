#!/bin/sh
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
wasm_cargo="$HOME/.cargo/bin/cargo"
CARGO_BUILD_JOBS=1 "$wasm_cargo" build --release --target wasm32-unknown-unknown --manifest-path "$project_root/dashboard-wasm/Cargo.toml"
"$HOME/.local/bin/wasm-bindgen" --target web --out-name iced --out-dir "$project_root/dashboard-wasm/generated" "$project_root/dashboard-wasm/target/wasm32-unknown-unknown/release/fortaleza_panel.wasm"
if [ -x /usr/bin/cargo ] && [ -x /usr/bin/rustc ]; then
  RUSTC=/usr/bin/rustc /usr/bin/cargo build --release --manifest-path "$project_root/collector/Cargo.toml" --features dashboard --bin fortaleza-dashboard
else
  cargo build --release --manifest-path "$project_root/collector/Cargo.toml" --features dashboard --bin fortaleza-dashboard
fi
