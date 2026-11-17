---
packages:
  takumi-core: minor
  takumi: minor
  takumi-cli: patch
---

# `takumi` loads the `@import` rules of a `<style media>` element

The `takumi` command used to drop the `@import` rules of a `<style>` element with a `media` attribute. It now loads them and applies them under that media, as Chrome does.

```html
<style media="print">
  @import "print.css";
</style>
```

- `ParsedHtml::styles` lists each `<style>` element with its `text` and `media` apart.
- `StyleElement::css` returns the text inside the `@media` rule that `ParsedHtml::css` holds.
