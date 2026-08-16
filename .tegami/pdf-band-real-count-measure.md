---
packages:
  "takumi-pdf":
    type: patch
---

### Measure a band with the page count the cut produced

A header or footer band was measured once with three-digit stand-in counters, so a counter wider than three digits could wrap and get clipped, and a narrow band reserved margin it never used. The band now re-measures with the real page count until its height settles, the same way content counters already converge.
