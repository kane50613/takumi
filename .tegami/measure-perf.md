---
packages:
  "takumi": minor
---

# Measure text once per node

Text measurement hashes a node's style once, and a text node's baseline comes from its cached measurement unless a height or line limit clamped it.
