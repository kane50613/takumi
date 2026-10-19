---
packages:
  takumi-core: patch
---

# Keep the box of an element that holds only white space

An element whose only content is white space, such as `<div style="height: 20px; background: green"> </div>`, now takes up space and paints its background and border. It used to vanish along with its text.
