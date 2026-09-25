#!/usr/bin/env bash
set -euo pipefail

workspace=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cargo_bin=$(command -v cargo || true)
if [[ -z "$cargo_bin" ]]; then
    cargo_directory=${CARGO_HOME:-"$HOME/.cargo"}
    for candidate in "$cargo_directory/bin/cargo" "$cargo_directory/bin/cargo.exe"; do
        if [[ -x "$candidate" ]]; then
            cargo_bin=$candidate
            break
        fi
    done
fi
if [[ -z "$cargo_bin" ]]; then
    printf '%s\n' 'Install stable Rust and the native build tools first.' >&2
    exit 1
fi

cd -- "$workspace"
"$cargo_bin" fmt --check
"$cargo_bin" clippy --locked --all-targets -- -D warnings
"$cargo_bin" test --locked
"$cargo_bin" build --locked --release
