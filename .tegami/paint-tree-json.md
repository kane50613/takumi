---
packages:
  "takumi-paint": minor
---

# Write a paint tree to JSON

`JSON.stringify(tree)` now writes a versioned `SerializedPaintTree`: the tree's nodes, steps, and effects by index, and every glyph's outline. A tree painted on a server can be handed to a browser or a worker as plain data. Font files are left out.
