---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A `break-inside: avoid` box stays on its page when only an absolute box inside it runs past

In a PDF, a `break-inside: avoid` box moved to the next page whole when an absolutely positioned box inside it started past the end of the page, even though the box itself fit. The box now stays where it fits, and only the absolute box continues on the next page, as Chromium prints it.

```html
<!-- The box fits at the bottom of a page, and the badge starts past it -->
<div style="position: relative; break-inside: avoid">
  Kept on the first page
  <div style="position: absolute; bottom: -40px">Badge</div>
</div>
```
