---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint rounded border sides as Chrome does

Rounded borders with mixed colours, styles or opacities now clip each side and cut their corners the way Chrome does. A collapsed table border now stays square even with `border-radius`, as the spec requires.
