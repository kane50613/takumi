import { fetchResources, extractResourceUrls, googleFonts } from "takumi-js/helpers";
import { extractEmojis } from "takumi-js/helpers/emoji";
import { fromJsx } from "takumi-js/helpers/jsx";
import wasm, { init, Renderer } from "takumi-js/wasm";
import type * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { evaluateCodeExports, renderReact } from "./evaluate";
import { messageSchema, type RenderMessageInput } from "./schema";

const fetchCache = new Map<string, ArrayBuffer>();
const fontCssCache = new Map<string, string>();

function postMessage(message: RenderMessageInput, transfer?: Transferable[]) {
  return self.postMessage(message, { transfer });
}

// Dogfood: load Google Font subsets by content. Each subset registers uniquely-named under
// its `subsetOf` family, so `font-family` routes per script and any leftover falls back. The
// variable weight axis lets any `font-weight` render with a real face instead of faux bold.
const GOOGLE_FONTS = [
  "Inter",
  "Noto Sans JP",
  "Noto Sans KR",
  "Noto Sans SC",
  "Noto Sans Arabic",
  "Noto Sans Hebrew",
  "Noto Sans Devanagari",
  "Noto Sans Thai",
].map((name) => ({ name, weight: "100..900" as const }));

let renderer: Renderer | undefined;

(async () => {
  await init({ module_or_path: wasm });

  renderer = new Renderer();

  postMessage({ type: "ready" });
})();

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

        postMessage({
          type: "preview-result",
          id: payload.id,
          html: renderToStaticMarkup(element),
          width: options.width,
          height: options.height,
          cssContents: effectiveStylesheets,
        });

        node = extractEmojis(node, options.emoji ?? "twemoji");

        const resourceUrls = extractResourceUrls(node);

        const [images, fonts] = await Promise.all([
          fetchResources(resourceUrls, { cache: fetchCache }),
          googleFonts({ families: GOOGLE_FONTS, cache: fontCssCache }),
        ]);

        const start = performance.now();
        const animationOptions = options.animation;
        const outputBuffer = await (animationOptions
          ? (() => {
              const format = animationOptions.format ?? "webp";
              const fps = animationOptions.fps ?? 30;
              return renderer.renderAnimation({
                scenes: [
                  {
                    node,
                    durationMs: animationOptions.durationMs,
                  },
                ],
                width: options.width ?? 1200,
                height: options.height ?? 630,
                format,
                quality: options.quality,
                devicePixelRatio: options.devicePixelRatio,
                images,
                stylesheets: effectiveStylesheets,
                fonts,
                fps,
              });
            })()
          : renderer.render(node, {
              ...options,
              stylesheets: effectiveStylesheets,
              images,
              fonts,
            }));
        const duration = performance.now() - start;

        postMessage(
          {
            type: "render-result",
            result: {
              status: "success",
              id: payload.id,
              outputBuffer,
              duration,
              node,
              outputFormat: animationOptions?.format ?? options.format ?? "png",
              options,
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
