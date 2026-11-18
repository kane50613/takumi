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

- `ParsedHtml::styles` lists each non-empty CSS `<style>` element in the HTML namespace that applies, so not one inside an inline `<svg>`, with its `text` and `media` apart. That is every untitled one, plus the titled ones in the preferred style sheet set.
- `StyleElement::css` returns the text inside the `@media` rule that `ParsedHtml::css` holds.
