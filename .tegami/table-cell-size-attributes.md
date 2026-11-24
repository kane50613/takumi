---
packages:
  takumi-core: patch
---

# Size table cells by their `width`, `height`, and `nowrap` attributes

A `<td>` or `<th>` now reads its `width`, `height`, and `nowrap` attributes, as in a browser. They used to be ignored.

- `width` and `height` set the cell's CSS `width` and `height`, in pixels or as a percentage. Content or another cell in the row can still make the cell larger.
- A zero, empty, or non-numeric `width` or `height` is ignored.
- `nowrap` keeps the cell's text on one line, whatever its value.
- CSS that sets the cell's `width`, `height`, or `white-space` still wins over the attributes.
