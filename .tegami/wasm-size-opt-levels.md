---
packages:
  npm:@takumi-rs/wasm:
    replay:
      - exit-prerelease(npm:@takumi-rs/wasm)
  npm:@takumi-rs/core:
    replay:
      - exit-prerelease(npm:@takumi-rs/core)
---

### Shrink the published binaries

Size-optimize the layout and shaping crates that never run per pixel, cutting
the published WASM and native binary size.
