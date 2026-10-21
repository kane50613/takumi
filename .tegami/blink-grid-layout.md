---
packages:
  takumi-core: minor
---

# Lay out grid containers with Chromium's grid algorithm

Grid containers now follow Chromium's grid layout for item placement, named lines in `repeat()`, track sizing, `fr` tracks, content alignment, baselines, and intrinsic sizes, so items land where Chrome puts them. Absolutely positioned children of a grid take their grid area as Chromium does. Line names after the last track of a `repeat()` are no longer dropped.
