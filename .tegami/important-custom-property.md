---
packages:
  "takumi": patch
---

# Honour `!important` on a custom property

`--gap: 8px !important` kept the marker inside the value, so `var(--gap)` substituted `8px !important` and every declaration reading it fell over. The marker now marks the declaration, which then takes part in the cascade like any other.
