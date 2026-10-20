---
packages:
  takumi-core: patch
---

# Match an SVG's `<style>` rules like the page's own CSS

An SVG image's `<style>` sheet and `style` attributes now go through the selector engine takumi uses for HTML. Selectors such as `:nth-child()`, `:not()`, `:is()`, `:lang()`, and `~` now apply, where they used to drop the rule.

An `@property` descriptor marked `!important` is now ignored, as in Chrome, where its value used to keep the annotation.
