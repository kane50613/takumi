---
packages:
  "takumi-pdf":
    type: patch
---

### Embed opaque PNG images without decoding them

A PNG with no alpha channel now goes into the PDF as its own compressed stream
instead of being decoded and recompressed. Paletted sources keep their palette
as an `/Indexed` colour space rather than widening every pixel to RGB.
