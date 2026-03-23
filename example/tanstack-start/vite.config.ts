import { cloudflare } from "@cloudflare/vite-plugin";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import { defineConfig } from "vite";

const config = defineConfig({
  plugins: [tanstackStart(), cloudflare()],
  // this is for simulating the Cloudflare Workers environment in dev mode, do not add any non-worker specific conditions here.
  ssr: {
    resolve: {
      conditions: ["worker", "module", "browser", "development|production"],
    },
  },
  resolve: {
    tsconfigPaths: true,
  },
});

export default config;
