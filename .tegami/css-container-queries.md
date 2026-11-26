---
packages:
  takumi-core: minor
---

# Support `@container` and `container-type`

`container-type`, `container-name`, and the `container` shorthand make a box a query container, and `@container` rules read its size, as in Chrome:

- A `size` or `inline-size` container takes no size from its content on the axes it contains, and starts a new formatting context, so its children's margins stay inside it.
- A query asks the nearest container that has its name and contains every axis it reads, and matches nothing without one.
- Conditions take `width`, `height`, `inline-size`, `block-size`, `aspect-ratio`, and `orientation`, with range syntax, as a comma-separated list, and an `em` reads the container's font size.
- A condition with a feature it does not know holds of no container, while the other conditions in its list still can.
- `@container` also nests in a style rule.
- Container units such as `cqw` and `cqi` read the nearest container that contains their axis, and the viewport without one.

```css
.sidebar {
  container: sidebar / inline-size;
}
@container sidebar (width > 300px) {
  .card {
    display: flex;
  }
}
```
