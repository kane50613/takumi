import { test } from "bun:test";
import { join } from "node:path";
import { render, renderSvg } from "takumi-js";
import { file, gzipSync, write } from "bun";
import type { ReactNode } from "react";

const kb = (bytes: number) => `${(bytes / 1024).toFixed(1)} KB`;
import BlogPostTemplate from "../src/templates/blog-post-template";
import ChangelogTemplate from "../src/templates/changelog-template";
import DocsTemplate from "../src/templates/docs-template";
import EventTemplate from "../src/templates/event-template";
import ProductCardTemplate from "../src/templates/product-card-template";
import QuoteTemplate from "../src/templates/quote-template";
import RepositoryTemplate from "../src/templates/repository-template";

function testRender(name: string, template: ReactNode) {
  test(name, async () => {
    const images = [
      {
        src: "takumi.svg",
        data: await file(
          join(import.meta.dirname, "..", "..", "assets", "images", "takumi.svg"),
        ).arrayBuffer(),
      },
    ];
    const options = { width: 1200, height: 630, images };

    const webp = await render(template, {
      ...options,
      format: "webp",
      dithering: "floyd-steinberg",
    });
    const svg = await renderSvg(template, options);

    const webpSize = webp.buffer.byteLength;
    const svgSize = Buffer.byteLength(svg);
    const svgGzip = gzipSync(svg).byteLength;

    console.log(`${name}: webp ${kb(webpSize)} | svg ${kb(svgSize)} (gzip ${kb(svgGzip)})`);

    const out = join(import.meta.dirname, "output", name);
    await write(`${out}.webp`, webp.buffer);
    await write(`${out}.svg`, svg);
  });
}

testRender(
  "docs-template",
  <DocsTemplate
    title="Fumadocs Integration"
    description="When will Fuma meet me in person? Hope we can meet in Japan! Culpa dolore eu ullamco aute exercitation sint aute nostrud qui tempor commodo ad culpa culpa. Laborum laboris eu laborum Lorem aliquip nulla nulla est proident eu. Officia deserunt aute ex quis exercitation ut. Irure cupidatat eu dolor Lorem eu aliquip mollit voluptate esse aute fugiat officia proident aliquip."
    icon={<img alt="Takumi" src="takumi.svg" tw="w-16 h-16" />}
    primaryColor="hsla(354, 90%, 54%, 0.3)"
    primaryTextColor="hsl(354, 90%, 60%)"
    site="Takumi"
  />,
);

testRender(
  "blog-post-template",
  <BlogPostTemplate
    title="The Future of Web Rendering with Rust and WebAssembly"
    author="Kane Wang"
    date="Nov 24, 2025"
    category="Engineering"
    avatar={
      <img
        alt="Avatar"
        src="https://avatars.githubusercontent.com/u/33802653?s=400&u=265f123fc40f34df69466e0a4368f64cc8837e2f&v=4"
        tw="w-full h-full object-cover rounded-full"
      />
    }
  />,
);

testRender(
  "product-card-template",
  <ProductCardTemplate
    productName="Takumi Pro"
    price="$299"
    description="The ultimate image generation engine for your next project. Blazing fast, type-safe, and built for scale."
    brand="Takumi"
    image={
      <img
        alt="Product"
        src="takumi.svg"
        style={{ width: "200px", height: "200px", objectFit: "contain" }}
      />
    }
  />,
);

testRender(
  "event-template",
  <EventTemplate
    name="Shipping Rust to the Browser: Wasm in Production"
    track="Workshop"
    datetime="Thu, Sep 18, 2026 · 10:00 AM PT"
    location="Online"
    hostName="Lin Clark"
    hostTitle="Principal Engineer, Fastly"
  />,
);

testRender(
  "quote-template",
  <QuoteTemplate
    quote="We replaced our Puppeteer farm with Takumi and cut OG render time from 800ms to 12ms."
    author="Sara Vieira"
    role="Staff Engineer"
    company="Vercel"
  />,
);

testRender(
  "repository-template",
  <RepositoryTemplate
    owner="vercel"
    name="satori"
    description="Enlightened library to convert HTML and CSS to SVG. Powers OG image generation across the web."
    stars="12.4k"
    forks="298"
    language="TypeScript"
    langColor="#3178c6"
  />,
);

testRender(
  "changelog-template",
  <ChangelogTemplate
    version="v2.4.0"
    date="June 15, 2026"
    headline="Faster fonts, leaner core"
    bullets={[
      { tag: "New", text: "Explicit Fonts API" },
      { tag: "Perf", text: "30% smaller Wasm bundle" },
      { tag: "Fixed", text: "Emoji baseline alignment" },
    ]}
  />,
);
