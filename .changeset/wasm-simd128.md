---
"@takumi-rs/wasm": patch
---

Build the published WASM binary with SIMD128: the CI `RUSTFLAGS` env was overriding the `.cargo/config.toml` target rustflags that enable it.
