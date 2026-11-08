---
packages:
  takumi-core: minor
---

# `linear()` easing

`animation-timing-function` takes `linear()` from CSS Easing 2, a curve of straight lines between its control points, so an animation can approximate a bounce or a spring. Points without a percentage spread evenly, and a progress past either end extends the outermost line, as in Chromium. `AnimationTimingFunction::LinearCurve` holds it.
