import { createRequire } from "node:module";
import tailwindcss from "@tailwindcss/vite";
import mdx from "fumadocs-mdx/vite";
import press from "fumapress/vite";
import type { Plugin } from "vite";
import { defineConfig } from "waku/config";

const require = createRequire(import.meta.url);

/** The playground's language service needs the JavaScript compiler; the root `typescript` is the Go port. */
function typescriptCompiler(): Plugin {
  return {
    name: "playground-typescript-compiler",
    applyToEnvironment: (environment) => environment.name === "client",
    resolveId: (source) => (source === "typescript" ? require.resolve("typescript-5") : undefined),
  };
}

export default defineConfig({
  vite: {
    // The render worker splits lazily-imported template modules (echarts) into
    // their own chunks, which an iife worker bundle would inline instead.
    worker: {
      format: "es",
    },
    ssr: {
      external: ["typescript", "twoslash", "shiki", "@takumi-rs/core"],
    },
    optimizeDeps: {
      exclude: ["lucide-react"],
    },
    resolve: {
      tsconfigPaths: true,
      dedupe: ["fumadocs-ui", "fumadocs-core"],
    },
    plugins: [press(), mdx(), tailwindcss(), typescriptCompiler()],
  },
});
