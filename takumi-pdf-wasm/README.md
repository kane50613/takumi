# Takumi PDF WebAssembly bridge

This internal crate exposes the Rust PDF renderer to JavaScript through wasm-bindgen. The public npm package, runtime entry points, and build commands live in [`takumi-pdf-js`](../takumi-pdf-js).

Keep the binding implementation and `src/dts-header.d.ts` in sync when changing options or return values. See the [binding conventions](../CONTRIBUTING.md#webassembly-binding-apis) and [PDF guides](https://takumi.kane.tw/docs/pdf).
