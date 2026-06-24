---
packages:
  "npm:@takumi-rs/helpers": patch
---

### Widen the React peer range and declare `engines`

Relax the `react` peer from `^19.2.5` back to `^18.0.0 || ^19.0.0`, matching
`react-dom` and dropping the peer warning on React 19.2.x patch releases. All
published packages now declare `engines.node: ">=18"`.
