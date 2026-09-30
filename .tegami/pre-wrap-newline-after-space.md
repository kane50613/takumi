---
packages:
  "takumi": patch
---

# Break at a newline that follows a space under `pre-wrap`

- Under `white-space: pre-wrap`, a newline right after a space ends the line. `"A \nB"` used to measure and render as one line.
