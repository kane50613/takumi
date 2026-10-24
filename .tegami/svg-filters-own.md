---
packages:
  takumi-core: minor
---

# Run SVG filters in takumi

SVG `filter` elements and `filter` functions on SVG elements now resolve through the CSS cascade and run in takumi, and the `resvg`, `usvg`, and `svgtypes` dependencies are gone. Text in an SVG image that uses a filter is drawn again. `feGaussianBlur` blurs like CSS `blur()`, `feDisplacementMap` moves pixels by its `scale` instead of its square, and `saturate()` and `contrast()` use the Filter Effects coefficients, so filtered pixels can differ by a few levels.
