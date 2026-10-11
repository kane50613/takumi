---
packages:
  "takumi": patch
---

# Encode animated PNG faster

APNG frames now deflate through the same parallel, tuned path as still PNGs, so a 24-frame 1200x630 animation encodes 5x faster and comes out slightly smaller. Frames, timing and looping are unchanged.
