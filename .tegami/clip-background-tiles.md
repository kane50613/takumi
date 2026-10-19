---
packages:
  takumi: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Clip a repeated background to its box

A repeated `background-image` whose tiles do not divide the box evenly, such as 15px tiles in a 96px box, now stops at the box's edge. Its last row and column of tiles used to paint past it.
