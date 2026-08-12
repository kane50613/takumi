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
// the frame synchronously instead of flashing through an async load.
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
// Noto Sans superfamily across scripts. Set font-family on the root directly —
// without Preflight nothing reads the font vars, so text would inherit the UA
// default. Override every font var since the render has no serif/mono of its
// own, and pin `color` to the engine's initial value.
const FONT_FAMILY = `${FONT_FAMILIES.map((name) => `"${name}"`).join(", ")}, ui-sans-serif, system-ui, sans-serif`;
const ROOT_CSS = `:root{color-scheme:light;color:#000;font-family:${FONT_FAMILY};--font-sans:${FONT_FAMILY};--font-serif:${FONT_FAMILY};--font-mono:${FONT_FAMILY};--default-font-family:${FONT_FAMILY};--default-mono-font-family:${FONT_FAMILY}}`;

function extractClasses(html: string) {
  const classes = new Set<string>();
  for (const match of html.matchAll(/class="([^"]*)"/g)) {
    for (const token of match[1].split(/\s+/)) if (token) classes.add(token);
  }
  return [...classes];
}

// The engine treats the tree's outermost node as the document root, so `rem`
// resolves against that node's own font size. Hoist it onto `<html>` and let
// `<body>` drop out of the box tree, otherwise `rem` here would resolve against
// the frame's untouched root instead.
function paintRoot(doc: Document, html: string, height: number | undefined, padding?: string) {
  const { documentElement, body } = doc;

  body.innerHTML = html;
  const root = body.firstElementChild;

  documentElement.removeAttribute("class");
  documentElement.removeAttribute("style");

  if (root) {
    for (const { name, value } of root.attributes) documentElement.setAttribute(name, value);
    body.replaceChildren(...root.childNodes);
  }

  body.style.display = "contents";
  // A fixed-size frame clips like the render does; the flowing one grows to fit
  // instead, so it must keep whatever overflow the tree asked for.
  if (height) documentElement.style.overflow = "hidden";
  if (padding) documentElement.style.padding = padding;
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
  const frameRef = useRef<HTMLIFrameElement>(null);
  const styleRef = useRef<{ frame: HTMLIFrameElement; style: HTMLStyleElement }>(undefined);
  // An iframe never sizes itself to its content, so the flowing layout measures
  // the painted document and grows the frame to match.
  const [contentHeight, setContentHeight] = useState(0);

  useLayoutEffect(() => {
    const frame = frameRef.current;
    const doc = frame?.contentDocument;
    if (!frame || !doc || !html) return;

    if (styleRef.current?.frame !== frame) {
      // @font-face has to reach the frame's own document; a sheet in the host
      // page never applies inside it.
      const link = doc.createElement("link");
      link.rel = "stylesheet";
      link.href = googleFontsCssUrl();
      const style = doc.createElement("style");
      doc.head.replaceChildren(link, style);
      styleRef.current = { frame, style };
    }

    const paint = () => {
      if (!compiler || styleRef.current?.frame !== frame) return;
      styleRef.current.style.textContent = [
        ROOT_CSS,
        compiler.build(extractClasses(html)),
        ...(cssContents ?? []),
      ].join("\n\n");
      paintRoot(doc, html, height, padding);
      // Reading the layout here forces a reflow, not a repaint: the frame is
      // painted once, after this effect returns.
      if (!height) setContentHeight(doc.documentElement.scrollHeight);
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
          <iframe
            ref={frameRef}
            title="Browser preview"
            sandbox="allow-same-origin"
            className="mx-auto block border bg-white"
            style={{ width, height: contentHeight, zoom: scale }}
          />
        )}
      </div>
    );
  }

  return (
    <div ref={ref} className="relative h-full min-w-0 overflow-hidden bg-muted/20">
      {html && (
        <iframe
          ref={frameRef}
          title="Browser preview"
          sandbox="allow-same-origin"
          className="border bg-white"
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            width,
            height,
            transform: `translate(-50%, -50%) scale(${scale})`,
          }}
        />
      )}
    </div>
  );
}
