---
packages:
  takumi-pdf:
    type: patch
---

### Bound repeating radial gradients

A repeating radial gradient whose stops all sit at one position expanded to millions of stops, since the period it tiles by collapsed to zero. The expansion now tiles at most 512 periods, stretching the period to keep covering the full radius.
