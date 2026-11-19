---
packages:
  takumi-core: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# An `offset-path` with huge coordinates renders instead of panicking

A `path()` in `offset-path` whose coordinates run into the quintillions, as generated or user-supplied paths can, panicked while measuring its length. It now measures, and a percentage `offset-distance` lands at that share of the whole path instead of near its start.
