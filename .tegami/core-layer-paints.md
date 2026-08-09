---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

### Ask a background layer once whether it paints

`BackgroundImage::paints` replaces the three spellings each backend had for the same question.

PDF used to treat a `url()` layer as unpaintable when built without the `images` feature, which skipped the whole background-image pass rather than that one layer.
