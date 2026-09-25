"use client";

import { compile } from "tailwindcss";
import themeCss from "tailwindcss/theme.css?raw";
import utilitiesCss from "tailwindcss/utilities.css?raw";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { FONT_FAMILIES, googleFontsCssUrl } from "~/playground/fonts";
import type { BrowserPreviewData } from "./use-render-worker";

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
<body><div id="mount" style="display:flex;width:100%;height:100%;padding:0"></div>
<script>
const mount = document.getElementById("mount");
const sheet = document.getElementById("sheet");

// Announced on parse and again whenever the page says hello, since either side
// can be the one that is ready first.
const announce = () => {
  const channel = new MessageChannel();

  channel.port1.onmessage = (paint) => {
    if (paint.data?.type !== "paint") return;
    sheet.textContent = paint.data.css;
    mount.innerHTML = paint.data.html;
    // Takumi treats any source containing "<svg" as inline SVG markup; a
    // browser needs it wrapped in a data URI.
    for (const img of mount.querySelectorAll("img")) {
      const src = img.getAttribute("src") ?? "";
      if (src.includes("<svg"))
        img.src = "data:image/svg+xml;utf8," + encodeURIComponent(src);
    }
  };
  parent.postMessage({ type: "ready" }, "*", [channel.port2]);
};

addEventListener("message", (event) => {
  if (event.source === parent && event.data?.type === "hello") announce();
});
announce();
</script>`;

type Paint = { type: "paint"; css: string; html: string };

function extractClasses(html: string) {
  const classes = new Set<string>();
  for (const match of html.matchAll(/class="([^"]*)"/g)) {
    for (const token of match[1].split(/\s+/)) if (token) classes.add(token);
  }
  return [...classes];
}

function useFitScale(width: number | undefined, height: number | undefined) {
  const ref = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || !width || !height) return;
    const measure = () => setScale(Math.min(el.clientWidth / width, el.clientHeight / height, 1));
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
  // The frame is blank until its first paint lands, which would flash white
  // over the pane, so delivering a paint is what counts as painted.
  const [hasPainted, setHasPainted] = useState(false);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (event.source !== frameRef.current?.contentWindow) return;
      if ((event.data as { type?: string } | null)?.type !== "ready") return;

      const [port] = event.ports;

      if (!port) return;

      portRef.current?.close();
      portRef.current = port;

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

  return { frameRef, hasPainted };
}

export default function BrowserPreview({ preview }: { preview: BrowserPreviewData | undefined }) {
  const { ref, scale } = useFitScale(preview?.width, preview?.height);
  const [paint, setPaint] = useState<Paint>();
  const { frameRef, hasPainted } = usePaintFrame(paint);

  useEffect(() => {
    if (!preview?.html) return;

    const { html, cssContents, theme = "" } = preview;
    let cancelled = false;

    // `theme` only reaches the compiler: a custom token has to be declared for
    // `bg-brand` to exist at all. Its value arrives with the rest of the CSS,
    // which is unlayered and comes last, the position the binding gives it.
    void loadCompiler(theme).then((compiler) => {
      if (cancelled) return;

      setPaint({
        type: "paint",
        css: [ROOT_CSS, compiler.build(extractClasses(html)), ...(cssContents ?? [])].join("\n\n"),
        html,
      });
    });

    return () => {
      cancelled = true;
    };
  }, [preview]);

  return (
    <div ref={ref} className="relative h-full min-w-0 overflow-hidden bg-muted/20">
      {preview?.html && (
        // `border-0` overrides the border an iframe carries by default, which
        // paints a light ring around the preview.
        <iframe
          ref={frameRef}
          title="Browser preview"
          sandbox="allow-scripts"
          srcDoc={FRAME_HTML}
          onLoad={() => frameRef.current?.contentWindow?.postMessage({ type: "hello" }, "*")}
          className="block border-0"
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            width: preview.width,
            height: preview.height,
            transform: `translate(-50%, -50%) scale(${scale})`,
            visibility: hasPainted ? undefined : "hidden",
          }}
        />
      )}
    </div>
  );
}
