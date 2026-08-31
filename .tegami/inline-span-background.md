---
packages:
  "takumi-core":
    type: minor
  "takumi-pdf":
    type: minor
---

### Paint inline span backgrounds

A `display: inline` span fills its `background-color` under the text, one
rounded fragment per line. Horizontal padding reserves space on the line and
the fragment grows by it, so badge and pill markup renders instead of
silently dropping the background.
