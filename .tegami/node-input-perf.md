---
packages:
  "takumi": minor
  "takumi-pdf": minor
---

# Read node trees faster

Each node's keys are read once instead of being buffered first. A key another node type owns that comes before `type` is now parsed, so a malformed one is an error instead of being ignored.
