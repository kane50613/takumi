"use client";

import { compile } from "tailwindcss";
import themeCss from "tailwindcss/theme.css?raw";
import utilitiesCss from "tailwindcss/utilities.css?raw";
import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
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
// the frame without flashing through an async load.
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
// Noto Sans superfamily across scripts. Set the family on `:root` directly —
// without Preflight nothing reads the font vars, so text would inherit the
// frame's default. Override every font var since the render has no serif/mono
// of its own.
const FONT_FAMILY = `${FONT_FAMILIES.map((name) => `"${name}"`).join(", ")}, ui-sans-serif, system-ui, sans-serif`;
const ROOT_CSS = `:root{color:#000;font-family:${FONT_FAMILY};--font-sans:${FONT_FAMILY};--font-serif:${FONT_FAMILY};--font-mono:${FONT_FAMILY};--default-font-family:${FONT_FAMILY};--default-mono-font-family:${FONT_FAMILY}}
html,body{margin:0;height:100%}`;

// The frame runs the rendered markup, which is the user's to write. `sandbox`
// without `allow-same-origin` puts it in an opaque origin, so an `onerror`
// handler smuggled through `dangerouslySetInnerHTML` cannot reach the docs
// origin. The document loads once and repaints over `postMessage`, so updates
// do not flash the way a swapped `srcdoc` would.
const FRAME_HTML = `<!doctype html>
<meta charset="utf-8">
<link rel="stylesheet" href="${googleFontsCssUrl()}">
<style id="sheet"></style>
<body><div id="mount"></div>
<script>
const reportHeight = () =>
  parent.postMessage({ type: "height", value: document.documentElement.scrollHeight }, "*");

addEventListener("message", (event) => {
  if (event.source !== parent || event.data?.type !== "paint") return;
  document.getElementById("sheet").textContent = event.data.css;
  const mount = document.getElementById("mount");
  mount.style.cssText = event.data.mountStyle;
  mount.innerHTML = event.data.html;
  reportHeight();
  document.fonts.ready.then(reportHeight);
});
parent.postMessage({ type: "ready" }, "*");
</script>`;

/** Guards against a frame that reports a height big enough to hang the layout. */
const MAX_FRAME_HEIGHT = 20000;

type Paint = { type: "paint"; css: string; html: string; mountStyle: string };

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

  useEffect(() => {
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

/** Repaints the frame, holding the last paint until its bootstrap says it is up. */
function usePaintFrame(paint: Paint | undefined) {
  const frameRef = useRef<HTMLIFrameElement>(null);
  const pendingRef = useRef<Paint>(undefined);
  const [isReady, setIsReady] = useState(false);
  const [contentHeight, setContentHeight] = useState<number>();

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (event.source !== frameRef.current?.contentWindow) return;

      // The frame runs untrusted markup, so nothing it sends is taken at its
      // word: a spoofed `ready` costs one repaint, and a height is clamped.
      const message = event.data as { type?: string; value?: unknown } | null;

      if (message?.type === "ready") {
        setIsReady(true);
        if (pendingRef.current) {
          frameRef.current?.contentWindow?.postMessage(pendingRef.current, "*");
        }
        return;
      }

      if (message?.type === "height" && typeof message.value === "number") {
        setContentHeight(Math.min(Math.max(message.value, 0), MAX_FRAME_HEIGHT));
      }
    };

    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  useEffect(() => {
    if (!paint) return;

    pendingRef.current = paint;
    if (isReady) frameRef.current?.contentWindow?.postMessage(paint, "*");
  }, [paint, isReady]);

  return { frameRef, contentHeight };
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
  const [paint, setPaint] = useState<Paint>();
  const { frameRef, contentHeight } = usePaintFrame(paint);

  useEffect(() => {
    if (!html) return;

    let cancelled = false;
    const build = () => {
      if (cancelled || !compiler) return;

      setPaint({
        type: "paint",
        css: [ROOT_CSS, compiler.build(extractClasses(html)), ...(cssContents ?? [])].join("\n\n"),
        html,
        mountStyle: `display:flex;width:100%;height:${height ? "100%" : "auto"};padding:${padding ?? "0"}`,
      });
    };

    if (compiler) {
      build();
    } else {
      void loadCompiler().then(build);
    }

    return () => {
      cancelled = true;
    };
  }, [html, cssContents, height, padding]);

  const frame = (style: CSSProperties) => (
    <iframe
      ref={frameRef}
      title="Browser preview"
      sandbox="allow-scripts"
      srcDoc={FRAME_HTML}
      className="border bg-white"
      style={style}
    />
  );

  // Without a height the pane scrolls the flow at page width, since the browser
  // cannot paginate the HTML the way the PDF renderer does.
  // Vertical padding only: `clientWidth` counts horizontal padding, so the
  // scaled page would end up that much wider than the pane.
  if (!height) {
    return (
      <div ref={ref} className="h-full min-w-0 overflow-auto bg-muted/20 py-4">
        {html &&
          frame({
            width,
            height: contentHeight ?? 0,
            zoom: scale,
            display: "block",
            margin: "0 auto",
          })}
      </div>
    );
  }

  return (
    <div ref={ref} className="relative h-full min-w-0 overflow-hidden bg-muted/20">
      {html &&
        frame({
          position: "absolute",
          top: "50%",
          left: "50%",
          width,
          height,
          transform: `translate(-50%, -50%) scale(${scale})`,
        })}
    </div>
  );
}
