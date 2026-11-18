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

- `ParsedHtml::links` lists each `<link rel="stylesheet">` that would load, with its `href` and `media`. It leaves out a `disabled` link and one whose `type` is not CSS.
- `ParsedHtml::base_href` holds the `href` of the first `<base>` that has one.
- `ParsedHtml::css_with_links` returns, in document order, the CSS of each non-empty CSS `<style>` element and each fetched link, and wraps a link's `media` in an `@media` rule.
- Titled and alternate style sheets follow the preferred style sheet set, as in Chrome. Untitled sheets that are not alternate always apply. Of the titled ones, only the preferred set applies. The set is named by whichever comes first in the document, a `<meta http-equiv="default-style">` with a non-empty `content` or a titled sheet that is not alternate.
