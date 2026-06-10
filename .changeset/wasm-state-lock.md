---
"@takumi-rs/wasm": patch
---

Hold renderer state behind a lock so all methods take `&self`, preventing a panic from permanently breaking the wasm-bindgen borrow flag.
