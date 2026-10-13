---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Keep the line strut when no font family matches

A line that holds only an image or another inline box keeps the default font's strut when its `font-family` matches no registered font, as Chrome does. Such lines used to come out a few pixels short.
