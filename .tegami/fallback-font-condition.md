---
packages:
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-paint: minor
  takumi-pdf: minor
---

# A WebAssembly bundle can leave out the built-in Geist font

`@takumi-rs/wasm`, `takumi-paint`, and `takumi-pdf` now import Geist, their built-in fallback font, from `@takumi-rs/helpers/fallback-font` instead of building it into the binary. If you load your own fonts, set the `takumi-no-fallback-font` export condition to leave Geist out and save about 25 KB (brotli). Without the condition, renders are unchanged.

```ts title="vite.config.ts"
import { defaultServerConditions, defineConfig } from "vite";

export default defineConfig({
  ssr: {
    resolve: {
      conditions: ["takumi-no-fallback-font", ...defaultServerConditions],
    },
  },
});
```

- The example above covers server code. A Vite client build takes the condition in `resolve.conditions`, beside `defaultClientConditions`.
- esbuild takes the condition in `conditions`, and webpack in `resolve.conditionNames` beside its default conditions.
- Node.js and Bun take `--conditions=takumi-no-fallback-font`.
- A Vite server build for Node.js reads the condition only in the packages it bundles, so add `@takumi-rs/helpers` and every Takumi package between it and your code to `ssr.noExternal`, or run Node.js with the flag.
- With the condition, a renderer starts with no fonts, and `clearFonts()` leaves none.
- `@takumi-rs/core` still builds Geist in and ignores the condition.
