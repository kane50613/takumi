---
packages:
  takumi-core: patch
---

# Read `ch` and normal line height from the font that draws

`ch`, `line-height: normal`, and an inline box's ascent now come from the same primary font the text draws with, at its variation instance. With `font-optical-sizing: auto`, `ch` used the parent's font size to pick the optical size, so a `10ch` box in a 64px Inter came out 11px wider than in Chrome.
