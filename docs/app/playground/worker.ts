import { googleFonts, prepareImages } from "takumi-js/helpers";
import { extractEmojis } from "takumi-js/helpers/emoji";
import { fromJsx } from "takumi-js/helpers/jsx";
import type { FetchedImage, Node } from "takumi-js/helpers";
import wasm, { init, Renderer } from "takumi-js/wasm";
import pdfWasm from "takumi-pdf/wasm-url";
// `no-init` over the entry that instantiates the module: the worker has the
// asset URL already, and that entry needs top-level await, which an iife worker
// bundle cannot have.
import initPdf, { PdfRenderer } from "takumi-pdf/no-init";
import type { JSXElementConstructor } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { evaluateCodeExports } from "./evaluate";
import { renderReact } from "./render-react";
import { FALLBACK_FONT_URL, FONT_FAMILIES } from "./fonts";
import { inspectPdf } from "./inspect-pdf";
import { keyframesToCss } from "./preview-css";
import { messageSchema, type OutputKind, type RenderMessageInput } from "./schema";

const DEFAULT_IMAGE_SIZE = { width: 1200, height: 630 };

type PdfOptions = NonNullable<PlaygroundOptions["pdf"]>;

/**
 * What the browser preview pane and the status bar need: the box to lay the
 * HTML out in, and a name for it. PDF output has no preview pane, so it carries
 * a name alone.
 */
type OutputGeometry = {
  width: number;
  height?: number;
  /** CSS `padding` shorthand mirroring the PDF page margin. */
  padding?: string;
  label: string;
};

function pdfLabel(pdf: PdfOptions): string {
  if (pdf.viewport) {
    const { width, height } = pdf.viewport;

    return `${width} × ${height ?? "auto"}`;
  }

  const name =
    typeof pdf.size === "object"
      ? `${Math.round(pdf.size.width)} × ${Math.round(pdf.size.height)}`
      : (pdf.size ?? "a4").toUpperCase();

  return pdf.landscape ? `${name} landscape` : name;
}

function outputGeometry(options: PlaygroundOptions): OutputGeometry {
  if (options.pdf) return { width: 0, label: pdfLabel(options.pdf) };

  const { width = DEFAULT_IMAGE_SIZE.width, height = DEFAULT_IMAGE_SIZE.height } = options;

  return { width, height, label: `${width} × ${height}` };
}

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

// The wasm module is fetched the first time a template asks for a PDF.
let pdfRenderer: Promise<PdfRenderer> | undefined;

function loadPdfRenderer() {
  pdfRenderer ??= initPdf({ module_or_path: pdfWasm }).then(() => new PdfRenderer());

  return pdfRenderer;
}

/** Everything a render needs beyond the tree itself, fetched once per request. */
async function loadResources(node: Node, stylesheets: string[]) {
  const [images, fonts] = await Promise.all([
    prepareImages<FetchedImage>({ node, fetchCache }),
    googleFonts(GOOGLE_FONTS).catch(() => undefined),
  ]);

  return {
    images,
    fonts: fonts ?? [FALLBACK_FONT_URL],
    stylesheets,
    notice: fonts ? undefined : "Google Fonts unreachable · Latin fallback",
  };
}

type Resources = Awaited<ReturnType<typeof loadResources>>;

type RenderInput = Omit<Resources, "notice"> & {
  renderer: Renderer;
  node: Node;
  options: PlaygroundOptions;
  geometry: OutputGeometry;
};

type Output = {
  buffer: Uint8Array;
  kind: OutputKind;
  format: string;
};

/** The options pick the backend: a document, a timeline, or a single frame. */
async function renderOutput({
  renderer,
  node,
  options,
  geometry,
  images,
  fonts,
  stylesheets,
}: RenderInput): Promise<Output> {
  if (options.pdf) {
    const pdf = await loadPdfRenderer();

    return {
      buffer: await pdf.render(node, {
        cssVariables: options.cssVariables,
        ...options.pdf,
        stylesheets,
        images,
        fonts,
      }),
      kind: "pdf",
      format: "pdf",
    };
  }

  if (options.animation) {
    const { durationMs, fps = 30, format = "webp" } = options.animation;

    return {
      buffer: await renderer.renderAnimation({
        scenes: [{ node, durationMs }],
        width: geometry.width,
        height: geometry.height ?? geometry.width,
        format,
        fps,
        quality: options.quality,
        devicePixelRatio: options.devicePixelRatio,
        keyframes: options.keyframes,
        cssVariables: options.cssVariables,
        images,
        fonts,
        stylesheets,
      }),
      kind: "animation",
      format,
    };
  }

  return {
    buffer: await renderer.render(node, { ...options, stylesheets, images, fonts }),
    kind: "image",
    format: options.format ?? "png",
  };
}

async function renderRequest(renderer: Renderer, id: number, code: string) {
  const { default: component, options } = evaluateCodeExports(code, renderReact);
  const element = renderReact.createElement(component as JSXElementConstructor<unknown>);
  const { node, stylesheets } = await fromJsx(element);
  const effectiveStylesheets = options.stylesheets ?? stylesheets;
  const geometry = outputGeometry(options);

  // A PDF renders pages, which a single HTML flow cannot stand in for, so the
  // playground gives the whole pane to the viewer instead.
  if (!options.pdf) {
    postMessage({
      type: "preview-result",
      id,
      html: renderToStaticMarkup(element),
      width: geometry.width,
      height: geometry.height,
      padding: geometry.padding,
      cssContents: options.keyframes
        ? [...effectiveStylesheets, keyframesToCss(options.keyframes)]
        : effectiveStylesheets,
      cssVariables: options.cssVariables,
    });
  }

  const emojified = extractEmojis(node, options.emoji ?? "twemoji");
  const resources = await loadResources(emojified, effectiveStylesheets);

  const { notice, ...renderResources } = resources;
  const start = performance.now();
  const output = await renderOutput({
    renderer,
    node: emojified,
    options,
    geometry,
    ...renderResources,
  });
  const duration = performance.now() - start;
  const inspection = output.kind === "pdf" ? await inspectPdf(output.buffer) : undefined;

  postMessage(
    {
      type: "render-result",
      result: {
        status: "success",
        id,
        outputBuffer: output.buffer,
        duration,
        outputKind: output.kind,
        outputFormat: output.format,
        label: geometry.label,
        inspection,
        notice,
      },
    },
    [output.buffer.buffer],
  );
}

self.onmessage = async (event: MessageEvent) => {
  const payload = messageSchema.parse(event.data);

  switch (payload.type) {
    case "render-request": {
      if (!renderer) throw new Error("WASM is not ready yet!");

      try {
        await renderRequest(renderer, payload.id, payload.code);
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
    case "watchdog": {
      const [port] = event.ports;

      if (port) {
        // The evaluated code shares this global but never sees `port`, so it
        // cannot answer a ping while it holds the event loop.
        port.onmessage = (ping: MessageEvent) => {
          if (ping.data?.type === "ping") port.postMessage({ type: "pong", id: ping.data.id });
        };
        port.start();
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
