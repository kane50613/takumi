import { defineConfig } from "vite";

export default defineConfig({
  ssr: { noExternal: true },
  build: { ssr: "src/node.ts", outDir: "out/vite-ssr" },
});
