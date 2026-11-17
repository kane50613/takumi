---
packages:
  takumi-core: minor
  takumi: minor
---

# `parse_html` reports the style sheets a document links

A host can now fetch the CSS a document links and resolve it as a browser does.

```rust
let parsed = parse_html(html, FromHtmlOptions::default())?;
// `fetch_css` is your own loader, which returns `Option<String>`
let css = parsed.css_with_links(|link| fetch_css(&link.href));
```

- `ParsedHtml::links` lists each `<link rel="stylesheet">`, with its `href` and `media`.
- `ParsedHtml::base_href` holds the `href` of the first `<base>` that has one.
- `ParsedHtml::css_with_links` returns the CSS of every `<style>` element and fetched link in document order, and wraps a link's `media` in an `@media` rule.
- Titled and alternate style sheets follow the preferred style sheet set, as in Chrome, so `css` keeps only the first set's `<style>` elements.
