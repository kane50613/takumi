---
packages:
  "cargo:takumi-core": minor
---

### Place the underline below descenders with text-underline-position

`text-underline-position` now parses and applies. `under` measures the underline from the font's descent rather than its underline metric, so it clears descenders. `auto` and `from-font` keep the font's underline metric, which is what the renderer already did. `left` and `right` are rejected, since they only mean something in vertical writing modes, which takumi does not support.
