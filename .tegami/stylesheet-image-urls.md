---
packages:
  takumi-core: minor
---

# List the images a stylesheet references

`StyleSheet::image_urls` lists the `url()`s its rules and keyframes reference, as `Node::image_urls` does for a node tree, so a caller can load them before rendering.
