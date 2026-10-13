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
      external: ["typescript", "twoslash", "shiki", "@takumi-rs/core", "monaco-editor"],
    },
    optimizeDeps: {
      exclude: ["lucide-react"],
    },
    resolve: {
      tsconfigPaths: true,
      dedupe: ["fumadocs-ui", "fumadocs-core"],
    },
    plugins: [press(), mdx(), tailwindcss()],
    environments: {
      rsc: {
        build: {
          rolldownOptions: {
            output: {
              codeSplitting: {
                groups: [
                  // Waku bundles client components by the server chunk that references them.
                  // These render in the root layout on every page, so their own chunk keeps
                  // the docs' MDX components out of the homepage's first load.
                  {
                    name: "root-client",
                    test: /[\\/](app[\\/]components[\\/]search|@vercel[\\/]analytics[\\/]dist[\\/]react|fumadocs-core[\\/]dist[\\/](framework[\\/]|link\.js))/,
                  },
                ],
              },
            },
          },
        },
      },
    },
  },
});
