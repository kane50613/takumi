---
packages:
  "takumi": minor
---

# Measure text once per node and reuse its baseline

A text node's measurement key is digested once per render instead of on every layout query, and its first and last baselines come from the cached measurement instead of a second layout. Output is unchanged.
