---
packages:
  takumi-core: minor
---

# Style SVG with `fill`, `stroke`, and the other SVG properties

Stylesheets now parse `fill`, `stroke` and its longhands, `marker`, `paint-order`, `stop-color`, `flood-color`, `lighting-color`, `text-anchor`, `dominant-baseline`, `baseline-shift`, `vector-effect`, `mask-type`, the shape geometry properties `x`, `y`, `cx`, `cy`, `r`, `rx`, `ry`, and `d`, and the other SVG properties Chrome supports. `ComputedStyle` holds them in new field groups, so code that builds a `ComputedStyle` with a full struct literal must add the new fields or end with `..Default::default()`.
