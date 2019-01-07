#!/bin/sh

set -ex

# Build the `hello_world.wasm` file using Cargo
cargo build --target wasm32-unknown-unknown

# Run the `wasm-bindgen` CLI tool to postprocess the wasm file emitted by the
# Rust compiler to emit the JS support glue that's necessary
wasm-bindgen ../../../target/wasm32-unknown-unknown/debug/piet_web_example.wasm --out-dir basic-web-static/dist

# Finally, package everything up using Webpack and start a server so we can
# browse the result
cd basic-web-static
npm install
npm run serve
