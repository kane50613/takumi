---
packages:
  takumi-raster: patch
---

# An image drawn one to one is copied, not filtered

A draw whose scale rounds to a hair off one and whose offset lies within a thousandth of a whole pixel now copies its rows. A cached photograph 4032 pixels wide drawn 1200 wide is the usual case: it used to pass through the bilinear filter on every render, which softened it by a 256th.
