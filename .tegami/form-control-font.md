---
packages:
  takumi-core: patch
  takumi-cli: patch
---

# Form controls and `--list-resources` no longer warn of a missing font

A page with a text field, such as an `<input>` or a `<textarea>`, warned that no face is loaded for `Arial`, since the default style of a text field named it, so `--deny-warnings` failed on any form. The default style now names `system-ui`, which draws in the same font when no Arial is loaded. `takumi --list-resources` no longer warns that an `@font-face` family is missing either.

```diff
  $ takumi image signup.html -o signup.png --deny-warnings
- warning: no face is loaded for font-family "Arial", so text falls back to the next family that has one. Load the font, or check the name in font-family.
```
