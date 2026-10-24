---
packages:
  takumi-core: minor
---

# Style SVG with `fill`, `stroke`, and the other SVG properties

Stylesheets now parse `fill`, `stroke` and its longhands, `marker`, `paint-order`, `stop-color`, `flood-color`, `lighting-color`, `text-anchor`, `dominant-baseline`, `baseline-shift`, `vector-effect`, `mask-type`, and the shape properties `x`, `y`, `cx`, `cy`, `r`, `rx`, `ry`, and `d`. `color-rendering` and `buffered-rendering` are not parsed. A `clip-path: path()` whose string is not valid path data is now an invalid declaration, as in Chrome. Code that builds a `ComputedStyle` with a full struct literal must add the new fields or end with `..Default::default()`.
