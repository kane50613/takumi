---
packages:
  takumi-core: patch
---

# Give `flex: <number>` a `0%` basis

A `flex` shorthand without a basis, such as `flex: 1`, `flex: 2 2`, or Tailwind's `flex-1`, now sets `flex-basis: 0%`, as Chromium computes it. It used to set `0px`, so a `flex: 1; min-height: 0` item in a column with no height collapsed to zero instead of keeping its content's height.
