import { defineConfig } from "nitro";

const exportConditions = process.env.NITRO_EXPORT_CONDITIONS?.split(",");

export default defineConfig({
  serverDir: "./",
  compatibilityDate: "2025-10-30",
  ...(exportConditions && { exportConditions }),
});
