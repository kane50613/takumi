---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw outlines as Chrome does

- An inline element's outline draws `double`, `groove`, `ridge`, `inset` and `outset`.
- An inline outline wraps the element's border box on each line, nested elements included. `plain <b>bold</b> text` now gets one.
- A wrapped inline outline follows Chrome's shape, rounds by `border-radius`, and shades 3D styles per edge.
- A one-line inline outline paints like a box outline.
- Inline outlines are no longer 1px narrower than Chrome's.
- Dashed and dotted outlines start each edge on a dash, and translucent ones no longer darken at the corners.
- A box without `border-radius` keeps square outline corners.
- PDF draws an inline outline as one contour across lines, at the element's opacity.
