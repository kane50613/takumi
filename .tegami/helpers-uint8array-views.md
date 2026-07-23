---
packages:
  "@takumi-rs/helpers": patch
---

### Hand font and image bytes to the bindings as Uint8Array views

Fetched fonts and images flowed to the bindings as bare ArrayBuffers, which the native binding copies. Wrapping them in a Uint8Array view costs nothing and takes the zero-copy path.
