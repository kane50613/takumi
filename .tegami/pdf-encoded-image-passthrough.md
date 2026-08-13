---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: minor
---

### Embed JPEG and WebP images

`images` took bytes in any raster format, but only PNG reached the page: a JPEG or a WebP failed the whole render. Both embed now, and a JPEG keeps its own compression instead of being decoded and re-encoded.
