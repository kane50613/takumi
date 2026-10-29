---
packages:
  takumi-pdf: minor
  takumi-paint: minor
---

# `takumi-pdf` and `takumi-paint` drop their deprecated options

`takumi-pdf` no longer takes `stylesheets`. Pass CSS through `css`, which takes one string or a list. The `subsetOf` and `subsetRank` font fields are gone from both packages. Register each subset under its family `name` with its `unicodeRange`.
