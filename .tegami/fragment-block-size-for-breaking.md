---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A flex row whose item margin runs past the page moves to the next page whole

In a PDF, a flex row whose item margin ran past the end of the page stayed on that page and spilled onto the next. The row now reaches as far as that margin, as in Chromium, and moves to the next page whole when it fits there.

```html
<!-- The item fits on the page, and its margin ends past it -->
<div style="display: flex">
  <div style="height: 80px; margin-bottom: 30px">Item</div>
</div>
```
