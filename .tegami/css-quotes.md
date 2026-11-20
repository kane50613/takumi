---
packages:
  takumi-core: patch
---

# `content` takes `open-quote` and `close-quote`, and `quotes` sets their marks

`content: open-quote` and `close-quote` write a quotation mark that follows the nesting depth, and `no-open-quote` and `no-close-quote` move the depth without a mark. Before, any of them dropped the whole `content` declaration. `quotes` takes `auto`, `none`, or pairs of strings, one pair for each depth.

```css
blockquote {
  quotes: "«" "»" "‹" "›";
}

blockquote::before {
  content: open-quote;
}
```

`auto` takes the marks of the element's `lang` from CLDR, as Chromium does: `„` `“` in German and `「` `」` in Japanese, and `“` `”` when the language has none.
