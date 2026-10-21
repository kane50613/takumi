# takumi-html

<!-- cargo-rdme start -->

Parse HTML markup into a takumi [`Node`] tree.

The `style`, `tw` (Tailwind classes), `class`, `id`, `dir`, and `lang` attributes carry
over. Elements get Chromium's default styles unless [`FromHtmlOptions`] sets other presets.
`<style>` and `<script>` elements are dropped, so pass CSS to the renderer as a stylesheet.

In the `takumi` crate, enable the `from-html` feature and call `takumi::from_html`.

```rust
use takumi_core::layout::node::Node;
use takumi_html::{FromHtml, FromHtmlOptions};

let node = Node::from_html("<div style=\"color:red\">Hi</div>", FromHtmlOptions::default())?;
```

<!-- cargo-rdme end -->
