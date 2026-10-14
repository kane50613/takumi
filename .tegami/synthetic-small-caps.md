---
packages:
  "takumi-core": minor
  "takumi": minor
---

# Synthesize small caps for fonts without them

`font-variant-caps` now synthesizes small capitals when the font lacks the caps feature, as Chrome does. Lowercase letters render as capitals at 70% size, so `small-caps` works with fonts like Geist. `font-synthesis: none` or the new `font-synthesis-small-caps: none` turns this off.
