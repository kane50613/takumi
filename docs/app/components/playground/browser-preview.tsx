"use client";

import { compile } from "tailwindcss";
import themeCss from "tailwindcss/theme.css?raw";
import utilitiesCss from "tailwindcss/utilities.css?raw";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
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

// Mirrors the binding's `variables_stylesheet`: the `--` prefix is optional,
// and an entry that would escape the `:root` rule is dropped.
function variableDeclarations(variables: Record<string, string> | undefined) {
  return Object.entries(variables ?? {})
    .map(([name, value]): [string, string] => [name.startsWith("--") ? name : `--${name}`, value])
    .filter(
      ([name, value]) =>
        !/[:;{}]/.test(name) &&
        !/[;{}]/.test(value) &&
        !value.includes("/*") &&
        !value.toLowerCase().includes("!important"),
    )
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([name, value]) => `${name}:${value};`)
    .join("");
}

// One compiler per variables set: `@theme` is what lets a custom token like
// `bg-brand` compile at all. Cached so a remount (e.g. mobile tab switch) can
// paint the frame without flashing through a fresh compile.
const compilers = new Map<string, Promise<{ build(candidates: string[]): string }>>();
function loadCompiler(declarations: string): Promise<{ build(candidates: string[]): string }> {
  let promise = compilers.get(declarations);
  if (!promise) {
    const input = declarations ? `${INPUT}\n@theme{${declarations}}` : INPUT;
    promise = compile(input, {
      base: "/",
      loadStylesheet: async (id, base) => ({ path: id, base, content: SOURCES[id] ?? "" }),
    });
    // Variables the theme parser rejects should not blank the preview.
    if (declarations) promise = promise.catch(() => loadCompiler(""));
    compilers.set(declarations, promise);
  }
  return promise;
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
// origin. The document loads once and repaints over a port its bootstrap hands
// out on the page's `hello`, so updates do not flash the way a swapped `srcdoc`
// would. The port dies with the document, so a frame that navigates itself away
// stops receiving paints.
const FRAME_HTML = `<!doctype html>
<meta charset="utf-8">
<link rel="stylesheet" href="${googleFontsCssUrl()}">
<style>:root{background:#fff}</style>
<style id="sheet"></style>
<body><div id="mount"></div>
<script>
const mount = document.getElementById("mount");
const sheet = document.getElementById("sheet");

// Announced on parse and again whenever the page says hello, since either side
// can be the one that is ready first.
const announce = () => {
  const channel = new MessageChannel();
  const port = channel.port1;
  const reportHeight = () =>
    port.postMessage({ type: "height", value: document.documentElement.scrollHeight });
  // Images and fonts land after the paint returns, so the height follows the
  // mount rather than being read once. Watching starts with the first paint,
  // which is what tells the page the frame has something to show.
  const observer = new ResizeObserver(reportHeight);

  port.onmessage = (paint) => {
    if (paint.data?.type !== "paint") return;
    sheet.textContent = paint.data.css;
    mount.style.cssText = paint.data.mountStyle;
    mount.innerHTML = paint.data.html;
    observer.observe(mount);
  };
  parent.postMessage({ type: "ready" }, "*", [channel.port2]);
};

addEventListener("message", (event) => {
  if (event.source === parent && event.data?.type === "hello") announce();
});
announce();
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

/** Repaints the frame, holding the last paint until its bootstrap hands over a port. */
function usePaintFrame(paint: Paint | undefined) {
  const frameRef = useRef<HTMLIFrameElement>(null);
  const portRef = useRef<MessagePort>(undefined);
  const pendingRef = useRef<Paint>(undefined);
  const [contentHeight, setContentHeight] = useState<number>();
  // The frame is blank until its first paint lands, which would flash white
  // over the pane. Delivery is what counts as painted: the frame's own report
  // rides a port a later announce may already have closed.
  const [hasPainted, setHasPainted] = useState(false);

  useEffect(() => {
    const onPortMessage = (event: MessageEvent) => {
      // The frame paints untrusted markup, so its numbers are clamped rather
      // than trusted.
      const message = event.data as { type?: string; value?: unknown } | null;

      if (message?.type === "height" && Number.isFinite(message.value)) {
        setContentHeight(Math.min(Math.max(Number(message.value), 0), MAX_FRAME_HEIGHT));
      }
    };

    const onMessage = (event: MessageEvent) => {
      if (event.source !== frameRef.current?.contentWindow) return;
      if ((event.data as { type?: string } | null)?.type !== "ready") return;

      const [port] = event.ports;

      if (!port) return;

      portRef.current?.close();
      portRef.current = port;
      port.onmessage = onPortMessage;
      port.start();

      if (!pendingRef.current) return;

      port.postMessage(pendingRef.current);
      setHasPainted(true);
    };

    window.addEventListener("message", onMessage);

    return () => {
      window.removeEventListener("message", onMessage);
      portRef.current?.close();
      portRef.current = undefined;
    };
  }, []);

  useEffect(() => {
    if (!paint) return;

    pendingRef.current = paint;

    if (!portRef.current) return;

    portRef.current.postMessage(paint);
    setHasPainted(true);
  }, [paint]);

  return { frameRef, contentHeight, hasPainted };
}

export default function BrowserPreview({
  html,
  width = 1200,
  height,
  padding,
  cssContents,
  variables,
}: {
  html: string | undefined;
  width?: number;
  /** Omitted for paged PDF: the pane grows with the content instead of clipping. */
  height?: number;
  padding?: string;
  cssContents?: string[];
  variables?: Record<string, string>;
}) {
  const { ref, scale } = useFitScale(width, height);
  const [paint, setPaint] = useState<Paint>();
  const { frameRef, contentHeight, hasPainted } = usePaintFrame(paint);

  useEffect(() => {
    if (!html) return;

    let cancelled = false;
    const declarations = variableDeclarations(variables);

    // The unlayered `:root` sheet comes after everything the compiler built, the
    // position the binding gives it, so a variable overriding a builtin token
    // (`--color-red-500`) wins in the pane the way it wins in the render.
    void loadCompiler(declarations).then((compiler) => {
      if (cancelled) return;

      setPaint({
        type: "paint",
        css: [
          ROOT_CSS,
          compiler.build(extractClasses(html)),
          ...(cssContents ?? []),
          ...(declarations ? [`:root{${declarations}}`] : []),
        ].join("\n\n"),
        html,
        mountStyle: `display:flex;width:100%;height:${height ? "100%" : "auto"};padding:${padding ?? "0"}`,
      });
    });

    return () => {
      cancelled = true;
    };
  }, [html, cssContents, variables, height, padding]);

  // `border-0` overrides the border an iframe carries by default, which paints
  // a light ring around the preview.
  const frame = (style: CSSProperties) => (
    <iframe
      ref={frameRef}
      title="Browser preview"
      sandbox="allow-scripts"
      srcDoc={FRAME_HTML}
      onLoad={() => frameRef.current?.contentWindow?.postMessage({ type: "hello" }, "*")}
      className="block border-0"
      style={{ ...style, visibility: hasPainted ? undefined : "hidden" }}
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
