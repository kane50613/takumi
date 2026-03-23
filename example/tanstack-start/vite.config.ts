import { cloudflare } from "@cloudflare/vite-plugin";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import { defineConfig } from "vite";

const config = defineConfig({
  plugins: [tanstackStart(), cloudflare()],
  resolve: {
    tsconfigPaths: true,
  },
});

export default config;
