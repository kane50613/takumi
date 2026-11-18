---
packages:
  takumi-pdf: patch
  takumi-paint: patch
---

# takumi-pdf and takumi-paint read their WebAssembly in Vite dev SSR from a package inside the root

With `ssr.noExternal`, the Vite dev server serves the WebAssembly binary from the package's own `pkg/` folder, and the first server render failed with `Unable to locate Takumi WASM asset for SSR`. The loaders now look there too, as `@takumi-rs/wasm` already does.
