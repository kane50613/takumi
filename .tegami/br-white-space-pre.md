---
packages:
  "cargo:takumi-core": patch
  "cargo:takumi-html": patch
  "npm:@takumi-rs/helpers": patch
---

### Honour per-element white-space when collapsing inline text

Inline whitespace collapsing read the block's white-space value for every span, so a `white-space: pre` child inside a normal-collapsing parent lost its spaces and line breaks. Each span now collapses against its own value. `<br>` also carries a `white-space: pre` preset, so its line break survives.
