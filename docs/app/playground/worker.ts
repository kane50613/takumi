import { googleFonts, prepareImages } from "takumi-js/helpers";
import { extractEmojis } from "takumi-js/helpers/emoji";
import { fromJsx } from "takumi-js/helpers/jsx";
import wasm, { init, Renderer } from "takumi-js/wasm";
import type { PdfRenderer } from "takumi-pdf";
import pdfWasm from "takumi-pdf/takumi_pdf_wasm_bg.wasm?url";
import type * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { evaluateCodeExports, renderReact } from "./evaluate";
import { FONT_FAMILIES } from "./fonts";
import { inspectPdf } from "./inspect-pdf";
import { messageSchema, type OutputKind, type RenderMessageInput } from "./schema";

const fetchCache = new Map<string, Promise<ArrayBuffer>>();

function postMessage(message: RenderMessageInput, transfer?: Transferable[]) {
  return self.postMessage(message, { transfer });
}

// Dogfood: load Google Font subsets by content. Each subset registers uniquely-named under
// its `subsetOf` family, so `font-family` routes per script and any leftover falls back. The
// variable weight axis lets any `font-weight` render with a real face instead of faux bold.
const GOOGLE_FONTS = FONT_FAMILIES.map((name) => ({ name, weight: "100..900" as const }));

let renderer: Renderer | undefined;

(async () => {
  await init({ module_or_path: wasm });

  renderer = new Renderer();

  postMessage({ type: "ready" });
})();

// The PDF engine is a second 3.7 MB wasm module, so it only loads once a template
// asks for one.
let pdfRenderer: Promise<PdfRenderer> | undefined;

function loadPdfRenderer() {
  pdfRenderer ??= import("takumi-pdf").then(async ({ default: initPdf, PdfRenderer }) => {
    await initPdf({ module_or_path: pdfWasm });
    return new PdfRenderer();
  });

  return pdfRenderer;
}

function declarationsToCss(declarations: object): string {
  return Object.entries(declarations)
    .map(
      ([property, value]) =>
        `${property.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}: ${value};`,
    )
    .join(" ");
}

/** Serializes structured keyframes into a `@keyframes` rule for the browser preview. */
function keyframesToCss(keyframes: NonNullable<PlaygroundOptions["keyframes"]>): string {
  const rules = Array.isArray(keyframes)
    ? keyframes.map((rule) => {
        const body = rule.keyframes
          .map(
            (frame) =>
              `${frame.offsets.map((offset) => `${offset * 100}%`).join(", ")} { ${declarationsToCss(frame.declarations)} }`,
          )
          .join(" ");
        return `@keyframes ${rule.name} { ${body} }`;
      })
    : Object.entries(keyframes).map(([name, offsets]) => {
        const body = Object.entries(offsets)
          .map(([offset, declarations]) => `${offset} { ${declarationsToCss(declarations)} }`)
          .join(" ");
        return `@keyframes ${name} { ${body} }`;
      });

  return rules.join("\n");
}

const PX_PER_MM = 96 / 25.4;
const PAGE_SIZES = {
  a4: { width: 210 * PX_PER_MM, height: 297 * PX_PER_MM },
  letter: { width: 8.5 * 96, height: 11 * 96 },
};
const DEFAULT_PAGE_MARGIN = 48;

type PdfOptions = NonNullable<PlaygroundOptions["pdf"]>;

function marginPadding(margin: PdfOptions["margin"]): string {
  if (margin === undefined) return `${DEFAULT_PAGE_MARGIN}px`;
  if (typeof margin === "number") return `${margin}px`;

  const { top = 0, right = 0, bottom = 0, left = 0 } = margin;
  return `${top}px ${right}px ${bottom}px ${left}px`;
}

/**
 * Page geometry for the browser pane and the status bar. A paged PDF has no
 * preview height: the pane shows the HTML as one continuous flow, since the
 * browser cannot paginate it the way the renderer does.
 */
function pdfGeometry(pdf: PdfOptions) {
  if (pdf.viewport) {
    const { width, height } = pdf.viewport;
    return { width, height, label: `${width} × ${height ?? "auto"}` };
  }

  const size = typeof pdf.size === "object" ? pdf.size : PAGE_SIZES[pdf.size ?? "a4"];
  const preset = typeof pdf.size === "object" ? undefined : (pdf.size ?? "a4");
  const width = Math.round(pdf.landscape ? size.height : size.width);
  const height = Math.round(pdf.landscape ? size.width : size.height);
  const name = preset ? preset.toUpperCase() : `${width} × ${height}`;

  return {
    width,
    label: pdf.landscape ? `${name} landscape` : name,
    padding: marginPadding(pdf.margin),
  };
}

self.onmessage = async (event: MessageEvent) => {
  const payload = messageSchema.parse(event.data);

  switch (payload.type) {
    case "render-request": {
      if (!renderer) throw new Error("WASM is not ready yet!");

      try {
        const { default: component, options } = evaluateCodeExports(payload.code, renderReact);
        const element = renderReact.createElement(
          component as React.JSXElementConstructor<unknown>,
        );
        let { node, stylesheets } = await fromJsx(element);
        const effectiveStylesheets = options.stylesheets ?? stylesheets;
        // The browser preview only understands CSS, so serialize any structured
        // keyframes (which the engine takes as an object) into a `@keyframes` rule.
        const previewStylesheets = options.keyframes
          ? [...effectiveStylesheets, keyframesToCss(options.keyframes)]
          : effectiveStylesheets;
        const geometry = options.pdf
          ? pdfGeometry(options.pdf)
          : {
              width: options.width ?? 1200,
              height: options.height ?? 630,
              label: `${options.width ?? 1200} × ${options.height ?? 630}`,
            };

        postMessage({
          type: "preview-result",
          id: payload.id,
          html: renderToStaticMarkup(element),
          width: geometry.width,
          height: "height" in geometry ? geometry.height : undefined,
          padding: "padding" in geometry ? geometry.padding : undefined,
          cssContents: previewStylesheets,
        });

        node = extractEmojis(node, options.emoji ?? "twemoji");

        const [images, fonts] = await Promise.all([
          prepareImages({ node, fetchCache }),
          googleFonts(GOOGLE_FONTS),
        ]);

        const pdf = options.pdf && (await loadPdfRenderer());
        const start = performance.now();
        const animationOptions = options.animation;
        const outputBuffer = await (pdf
          ? pdf.render(node, {
              ...options.pdf,
              stylesheets: effectiveStylesheets,
              images,
              fonts,
            })
          : animationOptions
            ? renderer.renderAnimation({
                scenes: [{ node, durationMs: animationOptions.durationMs }],
                width: options.width ?? 1200,
                height: options.height ?? 630,
                format: animationOptions.format ?? "webp",
                quality: options.quality,
                devicePixelRatio: options.devicePixelRatio,
                images,
                stylesheets: effectiveStylesheets,
                keyframes: options.keyframes,
                fonts,
                fps: animationOptions.fps ?? 30,
              })
            : renderer.render(node, {
                ...options,
                stylesheets: effectiveStylesheets,
                images,
                fonts,
              }));
        const duration = performance.now() - start;

        const outputKind: OutputKind = pdf ? "pdf" : animationOptions ? "animation" : "image";

        postMessage(
          {
            type: "render-result",
            result: {
              status: "success",
              id: payload.id,
              outputBuffer,
              duration,
              outputKind,
              outputFormat: pdf ? "pdf" : (animationOptions?.format ?? options.format ?? "png"),
              label: geometry.label,
              inspection: pdf ? inspectPdf(outputBuffer) : undefined,
            },
          },
          [outputBuffer.buffer],
        );
      } catch (error) {
        postMessage({
          type: "render-result",
          result: {
            status: "error",
            id: payload.id,
            message: error instanceof Error ? error.message : "Unknown error",
          },
        });
      }

      break;
    }
    case "ready":
    case "render-result":
    case "preview-result": {
      throw new Error("Respond message should not be sent from main window.");
    }
    default: {
      payload satisfies never;
    }
  }
};
