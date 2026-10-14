---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Check small-caps support in each run's own font and script

`font-variant-caps` now checks the font that draws each run, and that font's caps features for the run's script, as Chrome does. A fallback font keeps the small capitals it ships, and a font that lists them only for some scripts synthesizes them for the rest.
