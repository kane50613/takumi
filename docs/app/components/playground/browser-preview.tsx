"use client";

import { compile } from "tailwindcss";
import indexCss from "tailwindcss/index.css?raw";
import themeCss from "tailwindcss/theme.css?raw";
import utilitiesCss from "tailwindcss/utilities.css?raw";
import { useLayoutEffect, useRef, useState } from "react";
import { FONT_FAMILIES, googleFontsCssUrl } from "../../playground/fonts";

// Takumi's `tw` has no Preflight: it keeps UA presets and defaults to border-box.
// Drop Preflight's base layer, keep only box-sizing/border, so the pane matches the render.
const BASE = `@layer base{*,::after,::before,::backdrop,::file-selector-button{box-sizing:border-box;border:0 solid}}`;

function withoutPreflight(css: string) {
  const start = css.indexOf("@layer base {");
  if (start === -1) return css;
  let depth = 0;
  for (let i = css.indexOf("{", start); i < css.length; i++) {
    if (css[i] === "{") depth++;
    else if (css[i] === "}" && --depth === 0) return `${css.slice(0, start)}${css.slice(i + 1)}`;
  }
  return css;
}

const SOURCES: Record<string, string> = {
  tailwindcss: `${withoutPreflight(indexCss)}\n${BASE}`,
  "tailwindcss/theme.css": themeCss,
  "tailwindcss/preflight.css": BASE,
  "tailwindcss/utilities.css": utilitiesCss,
};

// Cache the resolved compiler so a remount (e.g. mobile tab switch) can paint
// the shadow synchronously instead of flashing through an async load.
let compiler: { build(candidates: string[]): string } | undefined;
let compilerPromise: Promise<void> | undefined;
function loadCompiler() {
  compilerPromise ??= compile(`@import "tailwindcss";`, {
    base: "/",
    loadStylesheet: async (id, base) => ({
      path: id,
      base,
      content: SOURCES[id] ?? SOURCES[`${id}.css`] ?? "",
    }),
  }).then((c) => {
    compiler = c;
  });
  return compilerPromise;
}

// Mirror the worker's font stack so the pane routes text to the same faces: Inter
// as the sans default, Noto per script for fallback. Override every font var since
// the render has no serif/mono of its own.
const FONT_FAMILY = `${FONT_FAMILIES.map((name) => `"${name}"`).join(", ")}, ui-sans-serif, system-ui, sans-serif`;
const HOST_CSS = `:host{--font-sans:${FONT_FAMILY};--font-serif:${FONT_FAMILY};--font-mono:${FONT_FAMILY};--default-font-family:${FONT_FAMILY};--default-mono-font-family:${FONT_FAMILY}}`;

// @font-face in a shadow root is ignored by Chrome, so load the same Google Font
// subsets the worker uses at document level; the `css2` sheet subsets on demand.
let fontLoaded = false;
function loadFont() {
  if (fontLoaded || typeof document === "undefined") return;
  fontLoaded = true;
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = googleFontsCssUrl();
  document.head.append(link);
}

function extractClasses(html: string) {
  const classes = new Set<string>();
  for (const match of html.matchAll(/class="([^"]*)"/g)) {
    for (const token of match[1].split(/\s+/)) if (token) classes.add(token);
  }
  return [...classes];
}

function useFitScale(width: number, height: number) {
  const ref = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setScale(Math.min(el.clientWidth / width, el.clientHeight / height, 1));
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [width, height]);

  return { ref, scale };
}

export default function BrowserPreview({
  html,
  width = 1200,
  height = 630,
  cssContents,
}: {
  html: string | undefined;
  width?: number;
  height?: number;
  cssContents?: string[];
}) {
  const { ref, scale } = useFitScale(width, height);
  const hostRef = useRef<HTMLDivElement>(null);
  const shadowRef = useRef<{ mount: HTMLElement; sheet: CSSStyleSheet }>(undefined);

  useLayoutEffect(() => {
    const host = hostRef.current;
    if (!host || !html) return;

    loadFont();

    if (!shadowRef.current) {
      const root = host.shadowRoot ?? host.attachShadow({ mode: "open" });
      const sheet = new CSSStyleSheet();
      root.adoptedStyleSheets = [sheet];
      const mount = document.createElement("div");
      mount.style.cssText = "width:100%;height:100%;display:flex";
      root.replaceChildren(mount);
      shadowRef.current = { mount, sheet };
    }

    const paint = () => {
      if (!compiler || !shadowRef.current) return;
      shadowRef.current.sheet.replaceSync(
        [HOST_CSS, compiler.build(extractClasses(html)), ...(cssContents ?? [])].join("\n\n"),
      );
      shadowRef.current.mount.innerHTML = html;
    };

    if (compiler) {
      paint();
      return;
    }
    let cancelled = false;
    loadCompiler().then(() => {
      if (!cancelled) paint();
    });
    return () => {
      cancelled = true;
    };
  }, [html, cssContents]);

  return (
    <div ref={ref} className="relative h-full min-w-0 overflow-hidden bg-muted/20">
      {html && (
        <div
          ref={hostRef}
          className="border"
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            width,
            height,
            transform: `translate(-50%, -50%) scale(${scale})`,
            overflow: "hidden",
          }}
        />
      )}
    </div>
  );
}
