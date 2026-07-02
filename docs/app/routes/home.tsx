"use client";

import { HomeLayout } from "fumadocs-ui/layouts/home";
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine-oniguruma.mjs";
import sh from "shiki/langs/sh.mjs";
import tsx from "shiki/langs/tsx.mjs";
import githubDarkDefault from "shiki/themes/github-dark-default.mjs";
import githubLightDefault from "shiki/themes/github-light-default.mjs";
import { CodeDemo } from "~/components/home/code-demo";
import { CTA } from "~/components/home/cta";
import { Features } from "~/components/home/features";
import { Filmstrip } from "~/components/home/filmstrip";
import { Hero } from "~/components/home/hero";
import { Showcase } from "~/components/home/showcase";
import { Seo } from "~/components/seo";
import { baseOptions } from "~/layout-config";

// Source of example/twitter-images/components/home-demo-card.tsx; keep in sync.
const CODE_SNIPPET = `export default function DemoCard() {
  return (
    <div tw="flex h-full w-full flex-col justify-between bg-[#16130f] p-14 text-white">
      <div tw="flex items-center justify-between">
        <span tw="text-2xl text-[#a8a29a]">takumi.kane.tw</span>
        <span tw="h-10 w-10 bg-[#ff4d4d]" />
      </div>
      <h1
        tw="text-7xl font-bold leading-tight"
        style={{
          backgroundClip: "text",
          backgroundImage: "linear-gradient(110deg, #fff 60%, #ff4d4d)",
          color: "transparent",
        }}
      >
        This card is the code beside it.
      </h1>
      <div tw="flex items-center justify-between text-2xl text-[#a8a29a]">
        <span>Rendered without a browser</span>
        <span>1200 × 630</span>
      </div>
    </div>
  );
}`;

const CTA_COMMAND = "bun i takumi-js";

const TITLE = "Takumi — Render JSX to images. Skip the browser.";
const DESCRIPTION =
  "Rust-powered image rendering engine. Write JSX, get pixels. Runs on Node, browsers, and Cloudflare Workers.";

const highlighter = await createHighlighterCore({
  themes: [githubDarkDefault, githubLightDefault],
  langs: [tsx, sh],
  engine: createOnigurumaEngine(import("shiki/wasm")),
});

const highlightedCodeDemo = {
  dark: highlighter.codeToHtml(CODE_SNIPPET, {
    lang: "tsx",
    theme: "github-dark-default",
  }),
  light: highlighter.codeToHtml(CODE_SNIPPET, {
    lang: "tsx",
    theme: "github-light-default",
  }),
};

const highlightedCta = {
  dark: highlighter.codeToHtml(CTA_COMMAND, {
    lang: "sh",
    theme: "github-dark-default",
  }),
  light: highlighter.codeToHtml(CTA_COMMAND, {
    lang: "sh",
    theme: "github-light-default",
  }),
};

export default function Home() {
  return (
    <HomeLayout className="overflow-x-hidden" {...baseOptions}>
      <Seo title={TITLE} description={DESCRIPTION} path="" />

      <Hero />
      <CodeDemo highlightedHtml={highlightedCodeDemo} />
      <Filmstrip />
      <Features />
      <Showcase />
      <CTA highlightedHtml={highlightedCta} />
    </HomeLayout>
  );
}
