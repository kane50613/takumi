---
packages:
  takumi-core: patch
---

# `grid-row` and `grid-column` resolve named lines

A grid item placed by a line name, such as `grid-column: content-start / content-end` or `span gutter`, now lands on the named line. `span name` used to count as `span 1`, and `span 0` and negative spans are rejected, as in Chromium.

```css
.page {
  display: grid;
  grid-template-columns: [full-start] 1fr [content-start] 60ch [content-end] 1fr [full-end];
}

.wide {
  grid-column: full;
}
```
