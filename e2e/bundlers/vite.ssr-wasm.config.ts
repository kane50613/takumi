import { defaultServerConditions, defineConfig } from "vite";

export default defineConfig({
  ssr: {
    noExternal: true,
    resolve: { conditions: ["takumi-wasm", ...defaultServerConditions] },
  },
  build: { ssr: "src/node.ts", outDir: "out/vite-ssr-wasm", ssrEmitAssets: true },
});
