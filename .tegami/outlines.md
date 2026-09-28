---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw outlines as Chrome does

- An inline element's `outline` draws `double`, `groove`, `ridge`, `inset` and `outset`, which used to paint nothing.
- Dashed and dotted outlines start each edge on a dash, and a translucent dashed outline no longer darkens where its dashes overlap at the corners.
- PDF draws the `outline` of an inline element as one contour across line breaks, at the element's opacity.
- An outline around a box without `border-radius` keeps square corners under `outline-offset` and `outline-width`.
- A one-line inline outline paints like a box outline, so its 3D styles shade like a border.
- An inline outline wraps the element's border box on each line, including its padding, border and nested elements. `plain <b>bold</b> text` used to get no outline at all.
- A wrapped `solid` or `double` inline outline rounds its corners by the element's `border-radius`.
- A wrapped inline outline follows Chrome's shape. Its inner edge shrinks from the outer contour, dashed and dotted outlines round their corners, and 3D styles shade each edge with aliased mitred corners.
