---
packages:
  takumi-core: patch
---

# Place outside list markers outside the list item's border box, as Chrome does

An outside marker now hangs from the list item's border edge, so padding or a border on the item moves the text but not the marker. A marker taller than the first line, such as a large `list-style-image`, now pushes the content down so the two share a baseline.
