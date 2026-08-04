---
packages:
  "cargo:takumi-core": minor
---

### Add corner-shape

`corner-shape` and its per-corner longhands parse and render across all backends: `round`, `squircle`, `bevel`, `scoop`, `notch`, `square`, and `superellipse(<number>)`. The shape applies wherever `border-radius` does — borders, backgrounds, box shadows, masks, and overflow clipping — and animates by the spec's interpolation. Corners use Chromium's two-cubic superellipse approximation, so a `squircle` here matches one in a browser.
