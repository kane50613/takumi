---
"takumi": patch
---

Skip painting zero-sized nodes instead of compositing them through a full-viewport offscreen canvas, fixing a severe slowdown for zero-sized nodes with `opacity`.
