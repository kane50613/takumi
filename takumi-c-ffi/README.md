# takumi-c-ffi

C FFI bindings for Takumi's renderer.

## Build

```bash
cargo build -p takumi-c-ffi --release
```

## Header

`include/takumi.h` is generated from Rust with `cbindgen` during build.

## JSON payloads

- `node_json`: same node shape used by `takumi-wasm` / `takumi-napi-core`.
- `options_json`: render options (`width`, `height`, `format`, `quality`, `drawDebugBorder`, `devicePixelRatio`, `fetchedResources`).
- `frames_json`: animation frame list (`[{ "node": ..., "durationMs": 120 }]`).

Byte arrays inside JSON (`data`) accept either:
- base64 strings
- integer arrays (`[137,80,78,...]`)
