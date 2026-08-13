---
packages:
  takumi-pdf:
    type: patch
---

### Decode JPEG, WebP and GIF images

`raster-images` pulled in krilla's decoders but left takumi-core's compiled out, so an `<img>` carrying anything but PNG bytes failed the whole render with `Unsupported(Format(Unknown))`. The feature now forwards `takumi-core/image-decoding`, the way `svg` already forwards `takumi-core/svg`.
