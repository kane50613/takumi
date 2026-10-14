---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Measure text ink once per text node

A text node with its own `display`, such as `flex`, no longer shapes and breaks its text a second time to measure ink bounds while the scene is built. Dashboards with many styled text labels render about 30% faster.
