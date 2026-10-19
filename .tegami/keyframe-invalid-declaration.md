---
packages:
  takumi-core: patch
---

# Keep a keyframe when one of its declarations is invalid

`StyleSheet::parse_loosy` dropped a whole keyframe when one declaration in it failed to parse. Now it drops only that declaration, as it already did in style rules.
