import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import { defineConfig } from "vite";

const config = defineConfig({
  plugins: [tanstackStart()],
  resolve: {
    tsconfigPaths: true,
  },
});

export default config;
