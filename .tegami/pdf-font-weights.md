---
packages:
  takumi-pdf:
    type: patch
---

### Render the weight the text asked for

A variable font is embedded at the coordinates the run was shaped at, so `font-weight` and `font-stretch` reach the page instead of the font's default instance. A face with no bold or oblique of its own gets the same synthesized ones the raster renderer applies.
