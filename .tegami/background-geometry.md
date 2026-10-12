---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Snap boxes to pixels as Chrome does

- Layout keeps exact positions and sizes. Backgrounds, borders, shadows, outlines, images and clips snap to whole pixels as they paint. `measure` reports the exact sizes.
- A line keeps its exact height: `line-height: 1.2` at 32px is 38.39px, not 39px.
- Background and mask tiles land on Chrome's pixels, including `round`, `space`, `cover` and `contain`.
- A tile outside the box, or under an opaque border, no longer paints.
- An image drawn through a rounded or clipped shape no longer shifts half a pixel and blurs.
- A solid edge between pixels, such as a decoration line, covers its edge pixels in proportion.
- A root with `display: flex`, `grid` or `flow-root` and no width fills the viewport.
- An ellipsis follows the last word, not the trailing space.
- In PDF, a repeating layer behind `background-clip: text` repeats across the whole text.
- A `background-clip: text` background no longer shows past its box through a text stroke, and no longer scales with `text-fit`.
