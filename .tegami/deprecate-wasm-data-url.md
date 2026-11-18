---
packages:
  "@takumi-rs/wasm": patch
  "@takumi-rs/core": patch
---

# Editors flag `renderAsDataUrl` and `@takumi-rs/wasm/no-bundler`

`Renderer.renderAsDataUrl` from `@takumi-rs/wasm` and the `@takumi-rs/wasm/no-bundler` entry keep working and will be removed in v3. `@takumi-rs/wasm/no-bundler` loads the same module as `@takumi-rs/wasm`.

The option types no longer list the internal `failedFontFiles` field, and `@takumi-rs/wasm` no longer exports the internal `RenderNeeds` type.

#### Migration

```diff
- import init, { Renderer } from "@takumi-rs/wasm/no-bundler";
+ import init, { Renderer } from "@takumi-rs/wasm";

- const url = await renderer.renderAsDataUrl(node, { format: "png" });
+ const bytes = await renderer.render(node, { format: "png" });
+ const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
```
