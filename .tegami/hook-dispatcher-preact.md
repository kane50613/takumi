---
packages:
  npm:@takumi-rs/helpers:
    type: minor
---

### Resolve hooks without react-dom and render Preact trees

`fromJsx` installs a server-semantics hook dispatcher instead of falling back
to `react-dom/server`, handles context providers and consumers natively, and
renders Preact subtrees through `preact-render-to-string`. The `react-dom`
peer dependency is gone; `preact` and `preact-render-to-string` are new
optional peers.
