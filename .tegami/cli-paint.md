---
packages:
  takumi-cli: minor
---

# `takumi paint` writes what a render draws, as text or JSON

`takumi paint card.html -o -` prints one line for each box, text, and image of the document: where it lands, what fills it, and whether it lies off the canvas or has no area. `--detail drawables` and `--detail full` add each thing a node draws and the paint order, and `--format json` writes the paint tree takumi-paint returns.
