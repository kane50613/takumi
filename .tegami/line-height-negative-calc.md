---
packages:
  takumi-core: patch
---

# A negative line-height calculation clamps to zero

`line-height: min(1, -1)` or `calc(1em - 30px)` at 20px used to keep its negative value, which pushed the text above where `line-height: 0` puts it. A math function's negative result now clamps to zero, as Chromium clamps a calculation to the property's range.
