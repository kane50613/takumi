---
packages:
  takumi-core: minor
  takumi: minor
  takumi-cli: patch
---

# A `<style media>` element's `@import` rules load

`ParsedHtml::styles` lists each `<style>` element with its text and `media` apart, and `StyleElement::css` wraps the text in the `@media` rule that `ParsedHtml::css` holds. The `takumi` command now loads the `@import` rules of a `<style>` element with a `media` attribute and applies them under that media, as Chrome does. Before, it dropped them.
