---
packages:
  takumi-svg: patch
---

# Draw SVG images in SVG output with the render's fonts

An `<img>` of an SVG is now written into SVG output as paths, so its `<text>` uses the fonts the render registered and matches the PNG output. It used to embed the original markup, which the viewer drew in its own fallback font. An SVG with a filter or an embedded bitmap is still embedded as before.
