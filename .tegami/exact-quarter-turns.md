---
packages:
  "takumi": patch
---

# Rotate by whole quarter turns exactly

`rotate: 90deg`, `180deg` and `270deg` now build an exact matrix, as Chrome does, instead of carrying a sine error near 1e-7. An image turned upside down then draws a row at a time instead of a pixel at a time, so a real card with one renders 36% faster. A few edge pixels change by one level.
