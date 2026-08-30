import tailwindcss from "@tailwindcss/vite";
import mdx from "fumadocs-mdx/vite";
import press from "fumapress/vite";
import { defineConfig } from "waku/config";

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
    plugins: [press(), mdx(), tailwindcss()],
  },
});
