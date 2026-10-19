---
packages:
  takumi-core: patch
---

# Keep a background whose shorthand names an attachment

`background: red url(a.png) no-repeat fixed` used to drop the whole declaration over `fixed`. The shorthand now accepts `scroll`, `fixed`, and `local`; a still image paints them alike.
