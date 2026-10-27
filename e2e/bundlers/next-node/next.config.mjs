import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));

export default {
  output: "standalone",
  serverExternalPackages: ["@takumi-rs/core"],
  outputFileTracingRoot: root,
  turbopack: { root },
};
