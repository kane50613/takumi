import { fetchResources, extractResourceUrls } from "takumi-js/helpers";
import { extractEmojis } from "takumi-js/helpers/emoji";
import { fromJsx } from "takumi-js/helpers/jsx";
import wasm, { init, Renderer } from "takumi-js/wasm";
import type * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { evaluateCodeExports, renderReact } from "./evaluate";
import { messageSchema, type RenderMessageInput } from "./schema";

const fetchCache = new Map<string, ArrayBuffer>();

function postMessage(message: RenderMessageInput, transfer?: Transferable[]) {
  return self.postMessage(message, { transfer });
}

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

        const fetchedResources = await fetchResources(resourceUrls, {
          cache: fetchCache,
        });

        const start = performance.now();
        const animationOptions = options.animation;
        const outputBuffer = animationOptions
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
                fetchedResources,
                stylesheets: effectiveStylesheets,
                fps,
              });
            })()
          : renderer.render(node, {
              ...options,
              stylesheets: effectiveStylesheets,
              fetchedResources,
            });
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
