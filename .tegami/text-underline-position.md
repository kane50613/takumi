---
packages:
  "cargo:takumi-core": minor
---

### Place the underline below descenders with text-underline-position

`text-underline-position` now parses and applies. `under` puts the underline at the bottom edge of the em box rather than at the font's underline metric, so it clears descenders. `auto` and `from-font` keep the font's underline metric, which is what the renderer already did. `left` and `right` are rejected, since they only mean something in vertical writing modes, which takumi does not support.
