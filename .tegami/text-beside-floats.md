---
packages:
  takumi-core: minor
---

# Wrap text around floats outside its block

Text now flows beside a float that an earlier block placed, as Chrome lays it out, instead of running over it. A `line-clamp` container sits beside such a float as a block formatting context of its own, and clips the floats inside it at its last shown line. A positioned box that was inline sits where Chrome puts it among blocks and floats, `text-align` and `text-indent` included.
