---
packages:
  "@takumi-rs/core":
    type: minor
---

### Support the legacy `page-break-*` properties

Print stylesheets written for `page-break-before`, `page-break-after`, and `page-break-inside` had those declarations dropped. They now drive the same pagination as `break-before`, `break-after`, and `break-inside`, and the forced-break keywords `always`, `left`, and `right` are accepted alongside `page`.
