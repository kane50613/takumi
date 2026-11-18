---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# `break-after: page` inside a flex or grid item starts a new page after the container

In a PDF, `break-after: page` on the last child of a flex or grid item, or on the last row of a table inside one, was dropped, so the content after the container stayed on the same page. The break now carries out through the item and the table, as Chromium prints it, and the content after the container starts a new page.

```html
<div style="display: flex">
  <div>
    <div>First</div>
    <div style="break-after: page">Ends with a forced break</div>
  </div>
</div>
<div>Starts a new page</div>
```
