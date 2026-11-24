---
packages:
  takumi-core: patch
  takumi-pdf: patch
  takumi-raster: patch
---

# Backgrounds continue across page breaks instead of restarting

A box that breaks across pages now paints its background image or gradient once down the whole box, as Chromium prints it, so each page goes on where the page before stopped. Before, the background restarted at the top of every page and fitted only that page's part of the box.

Under `box-decoration-break: clone`, the background still starts again on each page, now sized to the whole box as in Chromium.

A table's own background and a `background-clip: text` background still restart on each page.
