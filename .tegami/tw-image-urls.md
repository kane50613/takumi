---
packages:
  takumi-core: patch
---

# Collect images from Tailwind `list-image-*` and `content-*`

`Node::image_urls` now returns the images that `list-image-[url(...)]` and `content-[url(...)]` reference, as it does when the same properties are set in `style`. Before, it returned only the images from Tailwind `bg-*` and `mask-*`.
