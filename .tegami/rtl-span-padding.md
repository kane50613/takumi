---
packages:
  "takumi": patch
---

# Put a right-to-left inline span's start padding and border on its right

An inline span with `direction: rtl` now reserves its right padding, border and margin where it starts and its left ones where it ends, as in Chrome. Before, it used the left ones at its start, so uneven sides drew under the wrong text.
