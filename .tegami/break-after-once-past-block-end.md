---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# `break-after: page` breaks once when an absolute box inside runs onto the next page

In a PDF, when an absolutely positioned box ran past the page from inside a box with `break-after: page`, the break applied again on the next page, so a float after the box landed a page later than in Chromium. The break now applies once, where the box's content ends.

```html
<!-- The absolute box starts past the bottom of the page -->
<div style="position: relative; break-after: page">
  The box
  <div style="position: absolute; top: 110px">Runs onto the next page</div>
</div>
<div style="float: left">Starts the next page</div>
```
