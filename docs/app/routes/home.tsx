import { HomeLayout } from "fumadocs-ui/layouts/home";
import { useTheme } from "next-themes";
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine-oniguruma.mjs";
import sh from "shiki/langs/sh.mjs";
import tsx from "shiki/langs/tsx.mjs";
import githubDarkDefault from "shiki/themes/github-dark-default.mjs";
import githubLightDefault from "shiki/themes/github-light-default.mjs";
import { CodeDemo } from "~/components/home/code-demo";
import { CTA } from "~/components/home/cta";
import { Features } from "~/components/home/features";
import { Hero } from "~/components/home/hero";
import { Showcase } from "~/components/home/showcase";
import { baseOptions } from "~/layout-config";

const CODE_SNIPPET = `import { ImageResponse } from "@takumi-rs/image-response";

export async function GET() {
  return new ImageResponse(
    <div
      style={{
        display: "flex",
        background: "linear-gradient(135deg, #0a0a0a, #1a0a0a)",
        color: "white",
        padding: 48,
        width: "100%",
        height: "100%",
        fontFamily: "Geist",
      }}
    >
      <h1 style={{ fontSize: 64 }}>
        Hello, Takumi ✌️
      </h1>
    </div>,
  );
}`;

const CTA_COMMAND = "bun install @takumi-rs/image-response";

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
  const { resolvedTheme } = useTheme();
  const isLight = resolvedTheme === "light";

  return (
    <HomeLayout className="overflow-x-hidden" {...baseOptions}>
      <title>Takumi — Render your React components to images.</title>
      <meta
        name="description"
        content="Rust-powered image rendering engine. Write JSX, get pixels. Built for fast image generation across runtimes."
      />
      <meta name="og:title" content="Takumi — Render your React components to images." />
      <meta
        name="og:description"
        content="Rust-powered image rendering engine. Write JSX, get pixels. Built for fast image generation across runtimes."
      />
      <meta
        name="og:image"
        content="https://raw.githubusercontent.com/kane50613/takumi/master/example/twitter-images/output/og-image.png"
      />
      <meta
        name="twitter:image"
        content="https://raw.githubusercontent.com/kane50613/takumi/master/example/twitter-images/output/og-image.png"
      />

      <Hero />

      <section className="px-6 py-24 max-sm:py-12">
        <div className="max-w-[1100px] mx-auto">
          <div className="mb-14">
            <span className="inline-block text-xs font-semibold uppercase tracking-[0.12em] text-primary mb-3 px-3 py-1 rounded-full bg-primary/20">
              Bring Existing Code
            </span>
            <h2 className="font-display text-[clamp(2rem,4vw,3.2rem)] font-[750] tracking-tighter leading-tight mt-3">
              JSX in. Image out.
            </h2>
            <p className="text-[1.05rem] leading-relaxed text-muted-foreground max-w-[520px] mt-4">
              Write standard React components with CSS styling. Takumi renders them into
              production-quality images at blazing speed.
            </p>
          </div>
          <CodeDemo
            highlightedHtml={isLight ? highlightedCodeDemo.light : highlightedCodeDemo.dark}
          />
        </div>
      </section>

      <Features />
      <Showcase />
      <CTA highlightedHtml={isLight ? highlightedCta.light : highlightedCta.dark} />
    </HomeLayout>
  );
}
