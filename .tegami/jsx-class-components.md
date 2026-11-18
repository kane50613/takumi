---
packages:
  "@takumi-rs/helpers":
    type: patch
---

# JSX renders React class components

A component written as a class that extends `Component` or `PureComponent` threw `Class constructor cannot be invoked without 'new'` when rendered. It now renders as React mounts it: `defaultProps` fill missing props, `getDerivedStateFromProps` sets the first state, and `render` draws the element. This covers a class wrapped in `memo` and a class inside an inline `<svg>`.
