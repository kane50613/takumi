import { HomeLayout } from "fumadocs-ui/layouts/home";
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine-oniguruma.mjs";
import sh from "shiki/langs/sh.mjs";
import tsx from "shiki/langs/tsx.mjs";
import githubDarkDefault from "shiki/themes/github-dark-default.mjs";
import githubLightDefault from "shiki/themes/github-light-default.mjs";
import { CTA } from "~/components/home/cta";
import type { ThemedHtml } from "~/components/home/shiki-html";
import { Showcase } from "~/components/home/showcase";
import { Story } from "~/components/home/story";
import { Seo, SiteJsonLd } from "~/components/seo";
import * as motion from "../../../example/twitter-images/components/home-demo-card-motion";
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

// How example/twitter-images/animation.tsx renders home-demo-card-motion.tsx.
const KEYFRAMES_SNIPPET = `const webp = await renderAnimation({
  width: ${motion.width},
  height: ${motion.height},
  fps: ${motion.animation.fps},
  format: "webp",
  css: [morph], // "@keyframes morph { … }"
  scenes: [{ node: <DemoCardMotion />, durationMs: ${motion.animation.durationMs} }],
});`;

const CTA_COMMAND = "bun i takumi-js";

const TITLE = "Takumi: Open Graph images and PDFs from JSX";
const DESCRIPTION =
  "Generate Open Graph images, social cards, and PDF documents from JSX, HTML, and CSS. Render with Rust and WebAssembly without a headless browser.";

const highlighter = await createHighlighterCore({
  themes: [githubDarkDefault, githubLightDefault],
  langs: [tsx, sh],
  engine: createOnigurumaEngine(import("shiki/wasm")),
});

function highlight(code: string, lang: "tsx" | "sh"): ThemedHtml {
  return {
    dark: highlighter.codeToHtml(code, { lang, theme: "github-dark-default" }),
    light: highlighter.codeToHtml(code, { lang, theme: "github-light-default" }),
  };
}

const highlightedCard = highlight(CODE_SNIPPET, "tsx");
const highlightedKeyframes = highlight(KEYFRAMES_SNIPPET, "tsx");
const highlightedInstall = highlight(CTA_COMMAND, "sh");

export default function Home() {
  return (
    <HomeLayout className="overflow-x-clip" {...baseOptions}>
      <Seo title={TITLE} description={DESCRIPTION} path="" />
      <SiteJsonLd />

      <Story cardSource={highlightedCard} keyframes={highlightedKeyframes} />
      <Showcase />
      <CTA install={highlightedInstall} />
    </HomeLayout>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
