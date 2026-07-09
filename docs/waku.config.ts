import tailwindcss from "@tailwindcss/vite";
import mdx from "fumadocs-mdx/vite";
import press from "fumapress/vite";
import { defineConfig } from "waku/config";

export default defineConfig({
  vite: {
    ssr: {
      external: ["typescript", "twoslash", "shiki", "@takumi-rs/core"],
    },
    optimizeDeps: {
      exclude: ["lucide-react"],
    },
    resolve: {
      tsconfigPaths: true,
      dedupe: ["fumadocs-ui"],
    },
    plugins: [press(), mdx(), tailwindcss()],
  },
});
