"use client";

import { compile } from "tailwindcss";
import indexCss from "tailwindcss/index.css?raw";
import preflightCss from "tailwindcss/preflight.css?raw";
import themeCss from "tailwindcss/theme.css?raw";
import utilitiesCss from "tailwindcss/utilities.css?raw";
import { useEffect, useRef, useState } from "react";

const SOURCES: Record<string, string> = {
  tailwindcss: indexCss,
  "tailwindcss/theme.css": themeCss,
  "tailwindcss/preflight.css": preflightCss,
  "tailwindcss/utilities.css": utilitiesCss,
};

let compilerPromise: Promise<{ build(candidates: string[]): string }> | undefined;
function getCompiler() {
  compilerPromise ??= compile(`@import "tailwindcss";`, {
    base: "/",
    loadStylesheet: async (id, base) => ({
      path: id,
      base,
      content: SOURCES[id] ?? SOURCES[`${id}.css`] ?? "",
    }),
  });
  return compilerPromise;
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

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver(() =>
      setScale(Math.min(el.clientWidth / width, el.clientHeight / height, 1)),
    );
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

  useEffect(() => {
    const host = hostRef.current;
    if (!host || !html) return;

    if (!shadowRef.current) {
      const root = host.shadowRoot ?? host.attachShadow({ mode: "open" });
      const sheet = new CSSStyleSheet();
      root.adoptedStyleSheets = [sheet];
      const mount = document.createElement("div");
      mount.style.cssText = "width:100%;height:100%;display:flex";
      root.replaceChildren(mount);
      shadowRef.current = { mount, sheet };
    }

    let cancelled = false;
    void (async () => {
      const compiled = (await getCompiler()).build(extractClasses(html));
      if (cancelled || !shadowRef.current) return;
      shadowRef.current.sheet.replaceSync([compiled, ...(cssContents ?? [])].join("\n\n"));
      shadowRef.current.mount.innerHTML = html;
    })();
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
