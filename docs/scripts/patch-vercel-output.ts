/// Waku's Vercel adapter writes `.vercel/output/config.json` after `vercel build`, which drops
/// anything declared in `vercel.json`. The routes this site needs are merged back in here.
/// https://vercel.com/docs/build-output-api/v3/configuration#routes
import { readFileSync, writeFileSync } from "node:fs";

const CONFIG_PATH = ".vercel/output/config.json";

const MARKDOWN_ACCEPT = [{ type: "header", key: "accept", value: ".*[Tt]ext/[Mm]arkdown.*" }];

/// Collapsed to their final target: an agent that follows a chain sees the apex URL, not the hop.
const REDIRECTS: Record<string, string> = {
  "^/docs/getting-started(?:/.*)?$": "/docs",
  "^/docs/integrations(?:/.*)?$": "/docs",
  "^/docs/deep-dives(?:/.*)?$": "/docs/reference",
  "^/docs/persistent-images$": "/docs/load-images",
  "^/docs/tailwind-css$": "/docs/styling",
  "^/docs/quickstart$": "/docs",
  "^/docs/runtimes$": "/docs",
  "^/docs/layout-engine$": "/docs",
  "^/docs/integration/rust$": "https://docs.rs/takumi",
};

const beforeFilesystem = [
  ...Object.entries(REDIRECTS).map(([src, location]) => ({
    src,
    status: 301,
    headers: { Location: location },
  })),
  {
    src: "^/(.*)$",
    headers: { Vary: "Accept, Accept-Encoding" },
    continue: true,
  },
  {
    src: "^/(docs(?:/[\\w-]+)*)/?$",
    has: MARKDOWN_ACCEPT,
    dest: "/$1.md",
  },
];

const onError = [
  { handle: "error" },
  { src: "^/(?:r|api)(?:/.*)?$", status: 404, dest: "/errors/not-found.json" },
  { src: "^/.*$", has: MARKDOWN_ACCEPT, status: 404, dest: "/404.md" },
  { src: "^/.*$", status: 404, dest: "/404.html" },
];

export function withAgentRoutes(routes: unknown[]) {
  return [...beforeFilesystem, ...routes, ...onError];
}

if (import.meta.main) {
  const config = JSON.parse(readFileSync(CONFIG_PATH, "utf-8")) as { routes?: unknown[] };

  config.routes = withAgentRoutes(config.routes ?? []);

  writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2));

  console.log(`patched ${CONFIG_PATH} with ${beforeFilesystem.length + onError.length} routes`);
}
