---
packages:
  takumi-core:
    type: minor
  takumi-raster:
    type: minor
---

# Read what a render needs after the cascade

`RenderNode::needs` and `takumi_raster::needs` report the images a node tree looks up by URL and the characters its text shapes, read from the styled tree. Stylesheet and Tailwind rules, `display: none`, generated content, list markers, and `text-transform` count as they will render.
