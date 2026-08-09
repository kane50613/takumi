---
packages:
  takumi-pdf:
    type: patch
---

### Keep the lines inside a rotated or scaled box

Positions inside a transformed box are in that box's own frame, and pagination read them as positions on the page. A `break-before: page` inside one cut the page where nothing asked for a cut, and a line could end up claimed by no page at all and vanish from the document. A transformed box is now placed whole, as CSS fragmentation already treats it.
