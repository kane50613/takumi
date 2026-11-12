---
packages:
  takumi-core:
    type: minor
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
---

# An SVG image drawn in two text colors shows each color

An SVG image whose root sets `color="currentColor"` or inherits `color` through a `<style>` rule, or that nests an SVG image using `currentColor`, kept the raster of the first color it was drawn in, so the same icon next to red and blue text came out red both times. Each layout now records whether it read the host's color and fonts, and the cache keys the drawing on the values it read. `SvgSource::picture` takes over from `vector_ops`, which stays, deprecated. It returns the laid-out ops with the key that drawing shares, from the cache's new `pictures` budget.
