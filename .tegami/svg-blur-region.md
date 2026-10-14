---
packages:
  "takumi-svg": patch
---

# Stop clipping blurred shadows in SVG output

Blurred `box-shadow` and `text-shadow` no longer end in a hard-edged square. Each blur filter now covers the image plus the `3σ` the blur reaches, as Chrome does.
