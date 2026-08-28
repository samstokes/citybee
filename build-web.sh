#!/usr/bin/env bash
# Build the wasm bundle into web/, next to the index.html that loads it.
#
# Needs the wasm target and a matching wasm-bindgen CLI:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.127
#
# Then serve web/ over HTTP (opening index.html as a file:// URL will not
# work, browsers refuse to fetch wasm modules cross-origin):
#   python3 -m http.server -d web 8080
set -euo pipefail

cd "$(dirname "$0")"

PROFILE="${PROFILE:-release}"
case "$PROFILE" in
release) cargo_profile_flag=(--release) ;;
debug) cargo_profile_flag=() ;;
*)
    echo "PROFILE must be 'release' or 'debug', got '$PROFILE'" >&2
    exit 1
    ;;
esac

cargo build --target wasm32-unknown-unknown "${cargo_profile_flag[@]}"

bindgen_flags=(--no-typescript --target web --out-dir web --out-name citybee2)
if [ "$PROFILE" = release ]; then
    # The symbol name section is over half the module (~50MB of ~110MB). Keep it
    # in debug builds, where readable panic traces are worth the bytes.
    bindgen_flags+=(--remove-name-section)
fi

wasm-bindgen "${bindgen_flags[@]}" \
    "target/wasm32-unknown-unknown/$PROFILE/citybee2.wasm"
