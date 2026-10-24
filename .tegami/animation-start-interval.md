---
packages:
  takumi-core: patch
---

# Start a step animation at its first step

An animation at the very start of an iteration now samples the interval from its first keyframe, as in Chrome, so `steps(n, start)` shows its first step at time zero instead of the starting keyframe.
