---
packages:
  npm:takumi-js:
    type: patch
  npm:@takumi-rs/helpers:
    type: patch
---

### Bundle the WebContainer fallback instead of `@takumi-rs/wasm/auto`

The node backend's fallback pulled in `auto`, whose conditions resolve against
the host bundler: Turbopack sets `module` and got Vite's `?url` import of the
binary, failing the build. It now loads `@takumi-rs/wasm/node`, which every
bundler resolves, so only `@takumi-rs/core` needs externalizing.

### Drop `preact-render-to-string`

An optional import no bundler can skip: webpack and Vite have no optional
import, so resolving it statically failed the build of every app that renders
React and never installed it. Preact trees now traverse natively. Components
calling a Preact hook no longer render — those hooks live on Preact's mangled
internals, which no dispatcher can stand in for.

### Accept `preact/compat` elements in `ReactElementLike`

`preact/compat` types `$$typeof` as `symbol | string`, so its elements — and a
propless component's `FunctionComponentElement<never>` — no longer need a cast
to reach `render`.
