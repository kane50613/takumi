---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out and snap boxes to pixels as Chrome does

- Background and mask tiles are sized, positioned and snapped to pixels in 1/64px layout units, as Chrome does. `round`, `space`, `cover` and `contain` tiles now land on the same pixels as Chrome's.
- A tile that sits outside the box, or under an opaque border, no longer paints there.
- A repeating layer seen through `background-clip: text` in PDF repeats across the whole text instead of showing one tile.
- An image or background drawn through a rounded or clipped shape no longer shifts half a pixel and blurs in the image output.
- Layout keeps boxes at their exact positions and sizes. Backgrounds, borders, shadows, outlines, images and overflow clips snap to whole pixels as they paint, where Chrome snaps them. `measure` reports the unrounded sizes.
- A line of text keeps its exact height, so a `line-height: 1.2` line at 32px is 38.39px tall instead of 39px.
- A `background-clip: text` background no longer shows past its box through a text stroke, and no longer scales with `text-fit`.
- A root with `display: flex`, `grid` or `flow-root` and no width fills the viewport, as a block-level box does in Chrome.
- An ellipsis follows the last word directly instead of the line's trailing space.
- A solid rectangle whose edge falls between pixels, such as a decoration line or an inline box, covers its edge pixels in proportion in the image output, as the SVG output does.
