---
packages:
  "takumi": patch
---

# Keep a second moveto in SVG path data

SVG output no longer drops the letter of a moveto that follows another, which turned the second move into a stray line.
