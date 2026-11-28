---
packages:
  takumi-raster: patch
---

# Large and rect-shaped fills cover their pixels as in Chromium

Fills in raster output now cover the same pixels Chromium covers in a few more cases.

- A shape wider or taller than 8191 pixels fills across its whole width. Before, an aliased fill left everything past 16383 pixels empty, and an anti-aliased fill past 8191 pixels lost its anti-aliasing.
- A rect drawn with extra points along its sides, or without a closing segment, fills as a rect, so its edges can shade a level differently.
- A curve cut at the edge of the canvas splits at the point Chromium picks, which can move a shaded edge pixel by a level.
- A canvas stroke under a scale is outlined at the scale it is drawn, as Chromium does, so a scaled-up round cap or join keeps a round edge.
