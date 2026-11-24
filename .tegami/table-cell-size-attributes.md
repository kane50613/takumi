---
packages:
  takumi-core: patch
---

# Size table cells by their `width`, `height`, and `nowrap` attributes

A `<td>` or `<th>` with a `width` or `height` attribute now takes that size, in pixels or as a percentage, as in a browser. These attributes used to be ignored.

- `nowrap` keeps the cell's text on one line.
- A zero, empty, or non-numeric value is ignored.
- CSS that sets the cell's `width`, `height`, or `white-space` still wins over the attributes.
