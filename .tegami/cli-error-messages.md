---
packages:
  takumi-cli: patch
---

# `takumi` errors say how to fix more mistakes

- An image too tall for the canvas names the most height the width allows.
- An HTML file that holds binary data says it is not HTML, where it suggested converting its encoding.
- `--page` and `--margin` values takumi does not read come with examples of the values it does.
- `--selector` names an inline element without a background or border only when one matches, and otherwise says no element matches.

```diff
  $ takumi image tall.html -o tall.png
- error: the image would be 0px tall or over 64 megapixels at the content's height. Pass `--height`.
+ error: the image would be 0px tall or over 64 megapixels at the content's height, which at 1200px wide allows at most 55924px. Pass `--height`.
```
