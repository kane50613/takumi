---
"takumi": major
"takumi-core": minor
"takumi-raster": minor
---

Stop resolving `currentColor` in SVG images against the host color (SVG-as-image is isolated, matching browsers/satori); drop `current_color` from `ImageSource::render_for_layout`
