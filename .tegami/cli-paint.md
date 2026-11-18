---
packages:
  takumi-cli: minor
---

# `takumi paint` writes what a render draws, as text or JSON

`takumi paint` prints one line for each box, text, and image of a document, with where it lands and what fills it. A line also says when its node lies off the canvas, has no area, or draws nothing, and the elements that `opacity: 0` hides follow the outline.

```sh
$ takumi paint card.html -o - --width 300 --height 100
canvas 300x100
box html 0,0 300x121
  box body 0,0 300x121
    box div#card 0,0 300x100 background #0f172a clips-overflow
      box h1.title.big 0,0 300x41
        text h1.title.big 0,0 300x41 "Hello" Geist 700 32px #ffffff
      box div.ghost 900,0 20x20 background #ff0000 offscreen past the right edge
not painted
  section opacity: 0
```

- `--detail drawables` adds each thing a node draws.
- `--detail full` adds the paint order as well, and in JSON every glyph.
- `--format json`, or an output that ends in `.json`, writes the paint tree that takumi-paint returns.
