---
packages:
  takumi-html: patch
---

# Size images by percentage `width` and `height` attributes

`<img width="50%">` and `<svg width="100%" height="2em">` now take those sizes, as Chrome maps the attributes to style. A plain number of pixels sizes an image as before.
