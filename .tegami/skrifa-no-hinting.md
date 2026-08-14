---
packages:
  "@takumi-rs/wasm":
    type: patch
  takumi-pdf:
    type: patch
---

### Ship without skrifa's hinting interpreter

Every draw is unhinted, but skrifa's TrueType hinting interpreter and autohinter survived dead-code elimination through runtime branches. A patched skrifa gates them behind a `hinting` feature, cutting ~240KB from the wasm binaries with identical rendering.
