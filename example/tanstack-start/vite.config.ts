import { cloudflare } from "@cloudflare/vite-plugin";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import { defineConfig } from "vite";

const config = defineConfig({
  plugins: [
    tanstackStart(),
    cloudflare({
      viteEnvironment: {
        name: "ssr",
      },
    }),
  ],
  resolve: {
    tsconfigPaths: true,
  },
});

export default config;
