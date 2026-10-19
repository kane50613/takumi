---
packages:
  takumi-core: minor
---

# Support the `font` shorthand

`font: italic bold 16px/1.5 Geist, sans-serif` sets the style, weight, size, line height, and family in one declaration, and resets the other font properties it covers. System font keywords such as `caption` are not supported.

`font-weight` now rejects numbers outside 1 to 1000, and `font-family` rejects unquoted CSS-wide keywords and `default`, as browsers do.
