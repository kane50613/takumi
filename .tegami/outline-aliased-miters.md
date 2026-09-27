---
packages:
  "takumi": patch
---

# Cut 3D outline corners without antialiasing, as Chrome does

An `inset`, `outset`, `groove` or `ridge` outline around wrapped inline text now cuts its corner miters without antialiasing, as Chrome does. Before, a soft diagonal line showed where two shades meet.
