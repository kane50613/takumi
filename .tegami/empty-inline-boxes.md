---
packages:
  takumi-core: patch
---

# Draw inline boxes that have no content

An inline element with no content, such as `<span style="padding: 0 10px; background: red"></span>`, now paints its background, border, and outline, as browsers do. It used to take its space on the line but paint nothing.
