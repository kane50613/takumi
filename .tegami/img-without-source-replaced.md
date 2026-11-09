---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# An `<img>` without a source stays whole across pages

`from_html` turned an `<img>` with no `src` into an empty box, so a tall one split across two pages. It is now a replaced element with no pixels, as HTML renders an image that represents nothing, so it moves to the next page whole when it does not fit, as in Chromium. It lays out at the same size as before.
