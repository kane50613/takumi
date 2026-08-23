/// Waku's Vercel adapter writes `.vercel/output/config.json` from scratch, so the routes this site
/// needs are merged in after `vercel build` returns.
/// https://vercel.com/docs/build-output-api/configuration#routes
import { readFileSync, writeFileSync } from "node:fs";
import { normalizeRoutes } from "@vercel/routing-utils";
import type { HasField, Route } from "@vercel/routing-utils";

const CONFIG_PATH = ".vercel/output/config.json";

const MARKDOWN_ACCEPT: HasField = [
  { type: "header", key: "accept", value: ".*[Tt]ext/[Mm]arkdown.*" },
];

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

const beforeFilesystem: Route[] = [
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

const ERROR_PHASE = "error";

const onError: Route[] = [
  { handle: ERROR_PHASE },
  { src: "^/(?:r|api)(?:/.*)?$", status: 404, dest: "/errors/not-found.json" },
  { src: "^/.*$", has: MARKDOWN_ACCEPT, status: 404, dest: "/404.md" },
  { src: "^/.*$", status: 404, dest: "/404.html" },
];

/// A phase may be declared once, and every route after a `handle` belongs to it, so the error
/// routes are spliced into an existing `error` phase rather than appended blindly.
export function withAgentRoutes(routes: Route[]): Route[] {
  const errorPhase = routes.findIndex((route) => "handle" in route && route.handle === ERROR_PHASE);
  const [existingHandled, existingErrorRoutes] =
    errorPhase === -1 ? [routes, []] : [routes.slice(0, errorPhase), routes.slice(errorPhase + 1)];

  const merged = [...beforeFilesystem, ...existingHandled, ...onError, ...existingErrorRoutes];
  const { error } = normalizeRoutes(merged);

  if (error) throw new Error(`${error.message}\n${JSON.stringify(merged, null, 2)}`);

  return merged;
}

if (import.meta.main) {
  const config = JSON.parse(readFileSync(CONFIG_PATH, "utf-8")) as { routes?: Route[] };

  config.routes = withAgentRoutes(config.routes ?? []);

  writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2));

  console.log(`patched ${CONFIG_PATH}:\n${JSON.stringify(config.routes, null, 2)}`);
}
