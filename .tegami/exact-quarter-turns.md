---
packages:
  "takumi": patch
---

# Rotate by quarter turns exactly

`rotate: 90deg`, `180deg` and `270deg` build an exact matrix, as Chrome does. A card with an upside-down image renders 36% faster. A few edge pixels change by one level.
