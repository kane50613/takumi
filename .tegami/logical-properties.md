---
packages:
  takumi-core: minor
---

# Support the remaining logical properties

The block-axis longhands (`margin-block-start`, `inset-block-end`, …), the logical sizes (`inline-size`, `max-block-size`, …), and the logical borders and corner radii (`border-inline-start`, `border-block-color`, `border-start-end-radius`, …) now parse. Inline start and end follow `direction`, and `inset-inline` and `border-inline-width` now do too.
