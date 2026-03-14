---
"takumi": minor
---

**Migrate to builder pattern for constructing nodes**

Before:

```rust
let mut node = NodeKind::Container(ContainerNode {
  children: Some(Box::from([
    NodeKind::Text(TextNode {
      text: "Hello, world!".to_string(),
      style: None,
      tw: None,
      preset: None,
      tag_name: None,
      class_name: None,
      id: None,
    }),
  ])),
  preset: None,
  style: None,
  tw: None,
  tag_name: None,
  class_name: None,
  id: None,
});
```

After:

```rust
let node: NodeKind = ContainerNode::default()
  .with_child(TextNode::default().with_text("Hello, world!"))
  .into();
```
