---
packages:
  "cargo:takumi-core": patch
---

### Fix inherited resvg filter and parser bugs

Correct the spotlight Y offset, drop-shadow sRGB double conversion and displacement-map premultiplied reads; guard href cycles, turbulence seed overflow, convolve-matrix size overflow and oversized blur/morphology radii; make `<switch>` skip text branches and paint fallbacks apply for non-paint-server references.
