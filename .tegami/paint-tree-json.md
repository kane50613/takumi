---
packages:
  "takumi-paint": minor
---

# Write a paint tree to JSON and read it back

`JSON.stringify(tree)` now writes a versioned `SerializedPaintTree` that keeps every node, step, effect, and glyph outline. `paintTreeFromJSON(json)` reads it back into a `PaintTree` without loading the wasm module, so a tree painted on a server can be drawn in a browser or a worker. Font files are left out, so `font.data()` is empty on a tree read back this way.
