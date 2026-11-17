---
packages:
  takumi-core: minor
---

# An inline element's background and border are a box in the paint tree

In the paint tree, a `display: inline` element with a `background` or `border` now has a box node for each line it sits on, as a `display: inline-block` element already had. The box names the element and holds its `inline-background` and `border` drawables, which the enclosing text node used to hold.

```sh
$ takumi paint marked.html -o - --detail drawables
…
    box p 8,16 1184x21
      text p 8,16 1184x21 "Some marked text" Geist 16px #000000
        - glyphs text #000000 "Some " at 0,16 line 0 Geist 16px
        - glyphs text #000000 "marked" at 47.3,16 line 0 Geist 16px
        - glyphs text #000000 " text" at 103.48,16 line 0 Geist 16px
      box span 54.3,15 57.19x23 border
        - fill inline-background rect -0.3,0 57x23 #ffff00
        - fill border path #ff0000
```
