---
packages:
  "takumi": minor
  "takumi-pdf": minor
---

# Read node trees faster

Each node's keys are read once. A key that belongs to another node type and comes before `type` is now parsed, so a malformed one is an error.
