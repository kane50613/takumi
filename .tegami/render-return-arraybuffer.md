---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

### Type render output as backed by `ArrayBuffer`

`render` and `renderAnimation` declared their output as `Buffer` / `Uint8Array` over `ArrayBufferLike`, so passing the bytes straight to `new Response(...)` failed to typecheck. They now declare `Buffer<ArrayBuffer>` / `Uint8Array<ArrayBuffer>`, which `BodyInit` accepts.
