# takumi-pdf Rust backend

This internal Rust crate lays out and writes PDF documents with selectable text and embedded subset fonts. It is not published to crates.io.

For the JavaScript package named `takumi-pdf`, use [`takumi-pdf-js`](../takumi-pdf-js) and the [PDF documentation](https://takumi.kane.tw/docs/pdf). Its WebAssembly bridge, the `takumi-pdf-wasm` crate, lives in the same directory.

For changes to this backend, follow the [contribution guide](../CONTRIBUTING.md), run its Rust checks, and inspect PDF fixtures in `tests/fixtures-generated`.
