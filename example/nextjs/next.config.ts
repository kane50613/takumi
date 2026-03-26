import { join } from "node:path";
import type { NextConfig } from "next";

const config: NextConfig = {
  turbopack: {
    root: join(__dirname, "..", ".."),
  },
  serverExternalPackages: ["@takumi-rs/core"],
};

export default config;
