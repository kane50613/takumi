---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `display: contents` renders an element's children in its place

`display: contents` used to be dropped as an unknown value. An element with it now generates no box, and its children, `::before`, and `::after` lay out as children of its parent while they still inherit from the element.

```html
<!-- The flex container lays out One, Two, and Three as three items -->
<div style="display: flex">
  <div style="display: contents">
    <div>One</div>
    <div>Two</div>
  </div>
  <div>Three</div>
</div>
```

- A flex or grid container takes those children as its items.
- Replaced elements and form controls render nothing with it, as in browsers.
- The root element computes it to `block`, as in browsers.
- The Tailwind `contents` class sets it.
