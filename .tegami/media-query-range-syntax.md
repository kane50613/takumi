---
packages:
  "takumi-core":
    type: minor
---

### Accept media query range syntax

`@media (width >= 768px)` and `@media (400px < height <= 700px)` now parse
alongside `min-width` and `max-width`. A bare `(width)` matches any non-zero
viewport width.
