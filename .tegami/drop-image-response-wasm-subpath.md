---
packages:
  npm:@takumi-rs/image-response:
    type: patch
---

### Drop the `./wasm` export

`@takumi-rs/image-response/wasm` aliased the same file as the root entry. Import from
`@takumi-rs/image-response`.
