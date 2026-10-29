---
packages:
  takumi-core:
    type: minor
  takumi-raster:
    type: minor
---

# Read what a render needs after the cascade

`RenderNode::needs` and `takumi_raster::needs` report the images a node tree looks up by URL and the fonts its text would load, read from the styled tree. Stylesheet and Tailwind rules, `display: none`, generated content, list markers, `text-transform`, and the `text` elements of SVG images count as they will render.

Register a font that is still downloading with `Fonts::register_pending` and a `PendingFontFace`. Font matching treats it as a loading `@font-face`, and `needs` reports its id when the text reaches it before a loaded font with the glyph, as Blink starts loading a face.
