---
packages:
  takumi-cli: minor
---

# `takumi paint` writes what a render draws, as text or JSON

`takumi paint card.html -o -` prints one line for each box, text, and image of the document: where it lands, what fills it, and whether it lies off the canvas, has no area, or draws nothing. Elements that `opacity: 0` hides are listed after the outline. `--detail drawables` and `--detail full` add each thing a node draws and the paint order, and `--format json` writes the paint tree takumi-paint returns.
