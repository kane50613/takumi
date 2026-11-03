---
packages:
  takumi-paint: minor
---

# `PaintTree.free()` releases a tree's font files and glyph outlines

A paint tree keeps its font files and glyph outlines in WebAssembly memory until garbage collection releases them, which can take a while after many paints. `tree.free()` releases them right away. Calling it again does nothing, and the painter and other trees stay usable. Reading the files of a freed tree throws `The paint tree was freed`.
