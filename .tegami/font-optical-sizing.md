---
packages:
  "takumi-core": minor
  "takumi": minor
  "takumi-pdf": minor
---

# Support `font-optical-sizing`

Variable fonts with an `opsz` axis now set it to the font size, as Chrome does, so fonts like Inter draw their display cut at large sizes. `font-optical-sizing: none` or an `opsz` value in `font-variation-settings` turns this off.
