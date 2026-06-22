---
packages:
  "cargo:takumi": patch
  "cargo:takumi-core": patch
  "npm:takumi-js": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
  "npm:@takumi-rs/image-response": patch
---

### Keep font metadata when registering loaded fonts

`registerFont` passed only the resolved bytes to the engine, dropping each descriptor's
`name`/`subsetOf`/`weight`/`style`. Subsets that should register under unique names collapsed
onto their intrinsic family, so coverage variants were lost — text rendered as tofu, and which
variant survived depended on fetch-completion order, so the same content rendered differently
each run. Forward the descriptor so the override reaches the engine.

### Key the glyph cache by blob id instead of pointer

A freed font blob's address gets reused by a later font, aliasing its cached glyphs. Use the
blob's stable, never-reused id.
