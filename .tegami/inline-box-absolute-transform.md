---
packages:
  "takumi": patch
---

# Place a measured inline box against its parent's content box

An inline box's transform omitted the parent's border and padding, so it was reported at the border-box origin while every other measured node carries an absolute transform.
