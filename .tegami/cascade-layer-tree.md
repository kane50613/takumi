---
packages:
  takumi-core: patch
---

# Cascade layers, collapsed tables, box shadows, and universal custom properties render as in Chrome

- `@layer` orders its layers as a tree: a layer's own rules outrank its sublayers, the sublayers of one layer stay together, and each anonymous `@layer` block is a layer of its own.
- A table with `border-collapse: collapse` keeps its whole rounded `background`, since it paints no border of its own.
- A `box-shadow` with no offset, blur, or spread no longer paints.
- A registered custom property with `syntax: '*'` that is left without a value, through `initial`, an invalid `var()`, or a cycle, falls back in `var()` instead of taking its parent's or initial value.
- An animated length that mixes a container query unit with another unit resolves against its container.
