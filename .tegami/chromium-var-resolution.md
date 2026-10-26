---
packages:
  takumi-core: patch
---

# Align custom property resolution with Chromium

Custom-property aliases now resolve on the declaring element before inheritance. Cycles, fallbacks, and adjacent substituted tokens now follow Chromium's cascade behavior.
