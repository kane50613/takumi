---
packages:
  takumi-core:
    type: minor
  takumi-raster:
    type: minor
---

# Read what a render needs after the cascade

`RenderNode::needs` and `takumi_raster::needs` report the remote images a node tree draws and the characters its text shapes, read from the styled tree. Stylesheet and Tailwind rules, `display: none`, generated content, list markers, and `text-transform` count as they will render.
