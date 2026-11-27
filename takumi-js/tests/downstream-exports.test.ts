import { describe, expect, test } from "bun:test";
import { join } from "node:path";

// Subpaths nuxt-og-image and Fumadocs import. Removing one breaks them, so tell them first.
// https://github.com/nuxt-modules/og-image/blob/5f191b7dbed829a192cd66fffec7426c07fdc9c7/src/runtime/server/og-image/bindings/takumi/wasm.ts
// https://github.com/fuma-nama/fumadocs/blob/8364cb7ee14b4fa73f56684de6cf18fee8cd3272/packages/radix-ui/src/og/takumi.tsx
const imported = {
  "takumi-napi": ["."],
  "takumi-wasm": ["./no-bundler", "./takumi_wasm_bg.wasm"],
  "takumi-js": ["./response", "./helpers"],
};

describe("subpaths downstream packages import", () => {
  test.each(Object.entries(imported))("%s exports %p", async (dir, subpaths) => {
    const manifest = await Bun.file(join(import.meta.dir, "../..", dir, "package.json")).json();

    expect(Object.keys(manifest.exports)).toEqual(expect.arrayContaining(subpaths));
  });
});
