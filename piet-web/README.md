# Running the examples

Ensure both cargo and [npm] are installed.

Make sure that wasm-pack is installed:

`$ cargo install wasm-pack`

Then run the following:

`$ cd examples/basic && ./build.sh`

Then navigate your browser to the local web server that was started.

[npm]: https://www.npmjs.com/get-npm

# Testing

The easiest way is to use wasm-pack:

`$ cargo install wasm-pack`

Then use wasm-pack to run the tests:

`$ wasm-pack test --chrome --headless`

Tests are currently run only against chrome, once tests are made less brittle we'll also run against other browsers.

References:

- https://rustwasm.github.io/docs/wasm-bindgen/wasm-bindgen-test/index.html
- https://rustwasm.github.io/wasm-bindgen/wasm-bindgen-test/browsers.html
- https://rustwasm.github.io/docs/wasm-bindgen/wasm-bindgen-test/continuous-integration.html
