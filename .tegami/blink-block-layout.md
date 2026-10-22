---
packages:
  takumi-core: minor
---

# Place margins, floats, and positioned boxes where Chrome does

Block containers now follow Chromium's block layout for margin collapsing, floats, clearance, `position: relative`, and absolutely and fixed positioned boxes, so these land where Chrome puts them. Positions now round to Chromium's 1/64px layout unit, which can move text by up to 0.01px. Flex and grid containers still use taffy's algorithms.
