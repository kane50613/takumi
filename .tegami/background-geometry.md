---
packages:
  "takumi": patch
  "takumi-pdf": patch
  "takumi-paint": patch
---

# Lay out and snap boxes to pixels as Chrome does

- Background and mask tiles are sized, positioned and snapped to pixels in 1/64px layout units, as Chrome does. `round`, `space`, `cover` and `contain` tiles now land on the same pixels as Chrome's.
- A tile that sits outside the box, or under an opaque border, no longer paints there.
- A repeating layer seen through `background-clip: text` in PDF repeats across the whole text instead of showing one tile.
- An image or background drawn through a rounded or clipped shape no longer shifts half a pixel and blurs in the image output.
- Layout keeps boxes at their exact positions and sizes; backgrounds, borders, shadows, outlines, images and overflow clips snap to whole pixels as they paint, where Chrome snaps them. `measure` reports the unrounded sizes.
- `text-fit` leaves a line within 2px of its box unscaled, and a `grow` limit under 100% or a `shrink` limit over 100% stops the text from scaling, as Chrome does.
- A line of text keeps its exact height, so a `line-height: 1.2` line at 32px is 38.39px tall instead of 39px.
- `vertical-align` offsets land on the same 1/64px steps as Chrome's. A `top` or `bottom` box now aligns against the borders of the spans on its line.
- Spans on a `text-fit` line keep their `vertical-align` offsets and backgrounds where Chrome puts them, instead of scaling them a second time.
- Text on a `text-fit` line sits on the baseline Chrome paints it at, and its glyphs snap to the same pixel rows.
- Text inside a span aligned off the baseline, like `sub` or `super`, no longer kerns against the text around it, as in Chrome.
- A line with `line-height: normal` grows to fit the fonts its text falls back to, and a line with any other line height no longer does, as in Chrome.
- A `background-clip: text` background no longer shows past its box through a text stroke, and no longer scales with `text-fit`.
- Gradients sample each pixel at its centre, as Chrome does, so hard stops land on the same pixels. A tile of fractional size blends across its seams as Chrome's does.
- A `pattern` paint carries the `area` its tiles show in.
