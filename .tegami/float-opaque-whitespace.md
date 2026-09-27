---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Collapse the space after a float that starts a line

A float no longer keeps the space after it at the start of a line, as Chrome treats floats as invisible to white space collapsing.
