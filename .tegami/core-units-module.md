---
packages:
  takumi-core:
    type: minor
---

### Add `units` for the absolute length constants

`takumi_core::units` exports the CSS absolute units in 96 dpi pixels, so page geometry and CSS lengths resolve through one set of constants instead of each crate rederiving them.
