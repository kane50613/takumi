---
packages:
  "cargo:takumi": minor
  "cargo:takumi-core": minor
  "npm:takumi-js": minor
  "npm:@takumi-rs/core": minor
  "npm:@takumi-rs/helpers": minor
  "npm:@takumi-rs/wasm": minor
  "npm:@takumi-rs/image-response": minor
---

### Load only the Google Font subsets the content needs

`googleFontSubsets(content, families)` scans the codepoints a render uses, fetches every family's metadata in one css2 request, and keeps just the matching `unicode-range` subsets, so a multilingual image pulls a handful of CJK blocks instead of a whole font. Pass a `cache` Map to reuse the CSS across renders.

### Group coverage subsets under one logical family

`FontResource::subset_of` (Rust) and the `subsetOf` font field (JS) register a font as a subset of a logical family. A render expands `font-family: {logical}` into every subset registered under it, in order, so each script routes to the subset that covers it — distinct families no longer share a single fallback chain.
