---
packages:
  takumi-core: minor
---

# Lay out flex containers with Chromium's flex algorithm

Flex containers and `-webkit-box` now follow Chromium's flex layout for flex base sizes, automatic minimum sizes, line breaking, `flex-wrap: balance`, alignment, baselines, and intrinsic sizes, so items land where Chrome puts them. Absolutely positioned children of a flex container take Chromium's static position.
