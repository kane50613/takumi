import { reactRouter } from "@react-router/dev/vite";
import tailwindcss from "@tailwindcss/vite";
import mdx from "fumadocs-mdx/vite";
import { defineConfig } from "vite";
import * as MdxConfig from "./source.config";

export default defineConfig({
  ssr: {
    external: ["typescript", "twoslash", "shiki", "@takumi-rs/core"],
  },
  plugins: [mdx(MdxConfig), tailwindcss(), reactRouter()],
  resolve: {
    tsconfigPaths: true,
  },
});
