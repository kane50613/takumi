// Waku writes `.vercel/output/config.json` with only its own routes, so moved pages get their
// redirects added here after `waku build`. https://vercel.com/docs/build-output-api/configuration#routes
import { existsSync, readFileSync, writeFileSync } from "node:fs";

// The first match wins. A trailing `*` matches the rest of the path and carries it to a `*` target.
const redirects: [from: string, to: string][] = [
  ["/docs/integration/rust", "https://docs.rs/takumi"],
  ["/docs/integration", "/docs/frameworks"],
  ["/docs/integration/*", "/docs/frameworks/*"],
  ["/docs/comparison-to-satori", "/docs/migration/satori"],
  ["/docs/styling", "/docs/styles"],
  ["/docs/typography-and-fonts", "/docs/fonts"],
  ["/docs/load-images", "/docs/images"],
  ["/docs/visual-effects", "/docs/effects"],
  ["/docs/output-formats", "/docs/formats"],
  ["/docs/keyframe-animation", "/docs/animation"],
  ["/docs/performance-and-optimization", "/docs/performance"],
  ["/docs/image-response", "/docs/reference/image-response"],
  ["/docs/helpers", "/docs/reference/helpers"],
  ["/docs/measure-api", "/docs/reference/measure"],
  ["/docs/upgrade", "/docs/migration/v2"],
  ["/docs/upgrade/v2", "/docs/migration/v2"],
  ["/docs/upgrade/v1", "/docs/migration/v1"],
  ["/docs/pdf/headers-and-footers", "/docs/pdf/headers"],
  ["/docs/pdf/links-and-outline", "/docs/pdf/links"],
  ["/docs/pdf/single-page", "/docs/pdf/viewport"],
  ["/docs/pdf/fonts-and-images", "/docs/pdf/assets"],
  ["/docs/pdf/pdf-a", "/docs/pdf/conformance"],
  ["/docs/pdf/from-puppeteer", "/docs/pdf/migration/puppeteer"],
  ["/docs/pdf/from-react-pdf", "/docs/pdf/migration/react-pdf"],
  ["/docs/getting-started/*", "/docs/frameworks"],
  ["/docs/integrations", "/docs/frameworks"],
  ["/docs/integrations/*", "/docs/frameworks"],
  ["/docs/deep-dives", "/docs/reference"],
  ["/docs/deep-dives/*", "/docs/reference"],
  ["/docs/persistent-images", "/docs/images"],
  ["/docs/tailwind-css", "/docs/styles"],
  ["/docs/quickstart", "/docs"],
  ["/docs/runtimes", "/docs"],
  ["/docs/layout-engine", "/docs"],
];

const escape = (path: string) => path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

function route([from, to]: [string, string]) {
  if (from.endsWith("/*")) {
    return {
      src: `^${escape(from.slice(0, -2))}/(.*)$`,
      status: 301,
      headers: { Location: to.endsWith("/*") ? `${to.slice(0, -2)}/$1` : to },
    };
  }

  return { src: `^${escape(from)}/?$`, status: 301, headers: { Location: to } };
}

const config = ".vercel/output/config.json";

if (existsSync(config)) {
  const output = JSON.parse(readFileSync(config, "utf8"));

  output.routes = [...redirects.map(route), ...output.routes];
  writeFileSync(config, JSON.stringify(output, null, 2));
}
