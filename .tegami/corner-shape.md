---
packages:
  "cargo:takumi-core": minor
---

### Add corner-shape

`corner-shape` and its per-corner longhands render `round`, `squircle`, `bevel`, `scoop`, `notch`, `square`, and `superellipse(<number>)` corners, and interpolate in animations per the spec. The shape applies wherever `border-radius` does: borders, backgrounds, box shadows, masks, and overflow clipping. Corner curves use Chromium's superellipse approximation, so a squircle here matches one drawn by a browser.
