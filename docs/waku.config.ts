import tailwindcss from "@tailwindcss/vite";
import mdx from "fumadocs-mdx/vite";
import { defineConfig } from "waku/config";
import * as MdxConfig from "./source.config";

export default defineConfig({
  vite: {
    ssr: {
      external: ["typescript", "twoslash", "shiki", "@takumi-rs/core"],
      noExternal: ["waku", "react-server-dom-webpack"],
    },
    optimizeDeps: {
      exclude: ["lucide-react"],
    },
    resolve: {
      tsconfigPaths: true,
    },
    plugins: [tailwindcss(), mdx(MdxConfig)],
  },
});
