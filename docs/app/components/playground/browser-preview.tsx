"use client";

import { compile } from "tailwindcss";
import themeCss from "tailwindcss/theme.css?raw";
import utilitiesCss from "tailwindcss/utilities.css?raw";
import { useLayoutEffect, useRef, useState } from "react";
import { FONT_FAMILIES, googleFontsCssUrl } from "../../playground/fonts";

const SOURCES: Record<string, string> = {
  "tailwindcss/theme.css": themeCss,
  "tailwindcss/utilities.css": utilitiesCss,
};

// Takumi's `tw` has no Preflight: it keeps UA presets and defaults to border-box.
// Skip Preflight entirely; keep only its box-sizing/border reset so the pane
// matches the render.
const INPUT = `@layer theme, base, utilities;
@import "tailwindcss/theme.css" layer(theme);
@layer base{*,::after,::before,::backdrop,::file-selector-button{box-sizing:border-box;border:0 solid}}
@import "tailwindcss/utilities.css" layer(utilities);`;

// Cache the resolved compiler so a remount (e.g. mobile tab switch) can paint
// the shadow synchronously instead of flashing through an async load.
let compiler: { build(candidates: string[]): string } | undefined;
let compilerPromise: Promise<void> | undefined;
function loadCompiler() {
  compilerPromise ??= compile(INPUT, {
    base: "/",
    loadStylesheet: async (id, base) => ({ path: id, base, content: SOURCES[id] ?? "" }),
  }).then((c) => {
    compiler = c;
  });
  return compilerPromise;
}

// Mirror the worker's font stack so the pane routes text to the same faces: one
// Noto Sans superfamily across scripts. Set font-family on :host directly —
// without Preflight nothing reads the font vars, so text would inherit the docs
// page's font. Override every font var since the render has no serif/mono of
// its own. `color` is pinned to the engine's initial value: inherited properties
// cross the shadow boundary, so the docs theme would otherwise paint the pane's
// text white in dark mode.
const FONT_FAMILY = `${FONT_FAMILIES.map((name) => `"${name}"`).join(", ")}, ui-sans-serif, system-ui, sans-serif`;
const HOST_CSS = `:host{color:#000;font-family:${FONT_FAMILY};--font-sans:${FONT_FAMILY};--font-serif:${FONT_FAMILY};--font-mono:${FONT_FAMILY};--default-font-family:${FONT_FAMILY};--default-mono-font-family:${FONT_FAMILY}}`;

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

function useFitScale(width: number, height: number | undefined) {
  const ref = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () =>
      setScale(Math.min(el.clientWidth / width, height ? el.clientHeight / height : Infinity, 1));
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
  height,
  padding,
  cssContents,
}: {
  html: string | undefined;
  width?: number;
  /** Omitted for paged PDF: the pane grows with the content instead of clipping. */
  height?: number;
  padding?: string;
  cssContents?: string[];
}) {
  const { ref, scale } = useFitScale(width, height);
  const hostRef = useRef<HTMLDivElement>(null);
  const shadowRef = useRef<{ host: HTMLElement; mount: HTMLElement; sheet: CSSStyleSheet }>(
    undefined,
  );

  useLayoutEffect(() => {
    const host = hostRef.current;
    if (!host || !html) return;

    loadFont();

    // The host element is swapped when the pane switches between fixed-size and
    // flowing layouts, which leaves the old shadow root behind.
    if (shadowRef.current?.host !== host) {
      const root = host.shadowRoot ?? host.attachShadow({ mode: "open" });
      const sheet = new CSSStyleSheet();
      root.adoptedStyleSheets = [sheet];
      const mount = document.createElement("div");
      root.replaceChildren(mount);
      shadowRef.current = { host, mount, sheet };
    }

    const paint = () => {
      if (!compiler || !shadowRef.current) return;
      shadowRef.current.sheet.replaceSync(
        [HOST_CSS, compiler.build(extractClasses(html)), ...(cssContents ?? [])].join("\n\n"),
      );
      shadowRef.current.mount.style.cssText = `display:flex;width:100%;height:${height ? "100%" : "auto"};padding:${padding ?? "0"}`;
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
  }, [html, cssContents, height, padding]);

  // Without a height the pane scrolls the flow at page width, since the browser
  // cannot paginate the HTML the way the PDF renderer does.
  // Vertical padding only: `clientWidth` counts horizontal padding, so the
  // scaled page would end up that much wider than the pane.
  if (!height) {
    return (
      <div ref={ref} className="h-full min-w-0 overflow-auto bg-muted/20 py-4">
        {html && (
          <div ref={hostRef} className="mx-auto border bg-white" style={{ width, zoom: scale }} />
        )}
      </div>
    );
  }

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
