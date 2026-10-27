---
packages:
  takumi-core: patch
---

# List items inside `display: none` no longer take a number

An item hidden with `display: none`, or inside a hidden element, used to take the next number in its list, so the visible item after it skipped one. It now takes none, as Chromium numbers only items that get a box. Hidden subtrees are also no longer styled, which makes measuring a tree with a large hidden section about a third faster.
