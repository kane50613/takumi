---
"@takumi-rs/helpers": minor
---

**BREAKING CHANGE: `fromJsx()` now returns `{ node, stylesheets }`**

Before:

```tsx
const node = fromJsx(<div />);

renderer.render(node);
```

After:

```tsx
const { node, stylesheets } = fromJsx(<div />);

renderer.render(node, {
  stylesheets,
});
```
