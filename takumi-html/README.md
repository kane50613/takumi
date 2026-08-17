# takumi-html

<!-- cargo-rdme start -->

Parse HTML markup into a takumi [`Node`] tree.

```rust
use takumi_core::layout::node::Node;
use takumi_html::{FromHtml, FromHtmlOptions};

let node = Node::from_html("<div style=\"color:red\">Hi</div>", FromHtmlOptions::default())?;
```

<!-- cargo-rdme end -->
