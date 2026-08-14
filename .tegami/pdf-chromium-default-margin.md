---
packages:
  takumi-pdf:
    type: minor
---

### Start a page at the margin Chromium prints at

The default margin was 48px, a number with no source behind it. It is now the 1cm Chromium uses for `kDefaultMargins`, and an axis shorter than an inch keeps no margin at all, the way Chromium drops it rather than leave the page with nothing to print on. Pass `margin` to keep the old geometry.
