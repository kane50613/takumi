---
packages:
  takumi-raster: patch
---

# Large and rect-shaped fills cover their pixels as in Chromium

Fills in raster output now cover the same pixels Chromium covers in a few more cases. A shape wider or taller than 8191 pixels fills across its whole width, where an aliased fill used to leave everything past 16383 pixels empty and an anti-aliased one lost its anti-aliasing. A rect drawn with extra points along its sides or without a closing segment fills as a rect, a curve cut at the canvas edge splits where Chromium splits it, and a scaled canvas stroke keeps its round caps and joins round. Edges in these cases can shade a level differently.
