---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Turn off contextual alternates under letter spacing

Non-zero `letter-spacing` now also turns off contextual alternates (`calt`), as Chrome does. `font-variant-ligatures` can no longer turn ligatures back on while letters are spaced. `font-feature-settings` still overrides both.
