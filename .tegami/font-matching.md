---
packages:
  takumi-core: patch
---

# Match fonts as Chrome does

Within a family, a renderer now picks the face closest to `font-weight`, `font-stretch`, and `font-style` with the CSS font matching algorithm, as Chrome does. A variable face takes its `wght`, `wdth`, and `slnt` from the style, and a family split by `unicodeRange` keeps every subset for fallback.
