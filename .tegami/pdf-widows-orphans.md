---
packages:
  takumi-core:
    type: minor
  takumi-pdf:
    type: minor
---

### Honor `widows` and `orphans` at page breaks

A cut through a paragraph keeps at least `orphans` lines at the bottom of the page and `widows` lines at the top of the next. Both are inherited CSS properties and default to 2, the Chromium print default. Set both to 1 to disable the limits.
