---
packages:
  takumi-pdf:
    type: patch
---

### Draw images inside a wrapper

An image only reached the page when it was a direct child of the root. Wrapped in anything else, a `<div>`, a `<figure>`, or a plain container node, it laid out at the right size and then drew nothing. Images now paint from the inline layout they belong to, the same way the raster renderer already did.
