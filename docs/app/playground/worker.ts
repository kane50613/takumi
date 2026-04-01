import { fetchResources, extractResourceUrls } from "takumi-js/helpers";
import { extractEmojis } from "takumi-js/helpers/emoji";
import { fromJsx } from "takumi-js/helpers/jsx";
import wasm, { init, Renderer } from "takumi-js/wasm";
import * as React from "react";
import { transform } from "sucrase";
import * as z from "zod/mini";
import { messageSchema, optionsSchema, type RenderMessageInput } from "./schema";

const fetchCache = new Map<string, ArrayBuffer>();

function postMessage(message: RenderMessageInput) {
  return self.postMessage(message);
}

const exportsSchema = z.object({
  default: z.function(),
  options: optionsSchema,
});

let renderer: Renderer | undefined;

(async () => {
  await init({ module_or_path: wasm });

  renderer = new Renderer();

  postMessage({ type: "ready" });
})();

function transformCode(code: string) {
  return transform(code, {
    transforms: ["jsx", "typescript", "imports"],
    production: true,
  }).code;
}

function evaluateCodeExports(code: string) {
  const exports = {};

  new Function("exports", "React", transformCode(code))(exports, React);

  return exportsSchema.parse(exports);
}

self.onmessage = async (event: MessageEvent) => {
  const payload = messageSchema.parse(event.data);

  switch (payload.type) {
    case "render-request": {
      if (!renderer) throw new Error("WASM is not ready yet!");

      try {
        const { default: component, options } = evaluateCodeExports(payload.code);
        let { node, stylesheets } = await fromJsx(
          React.createElement(component as React.JSXElementConstructor<unknown>),
        );

        node = extractEmojis(node, options.emoji ?? "twemoji");

        const resourceUrls = extractResourceUrls(node);

        const fetchedResources = await fetchResources(resourceUrls, {
          cache: fetchCache,
        });

        const start = performance.now();
        const effectiveStylesheets = options.stylesheets ?? stylesheets;
        const animationOptions = options.animation;
        const outputUrl = animationOptions
          ? (() => {
              const format = animationOptions.format ?? "webp";
              const fps = animationOptions.fps ?? 30;
              const bytes = renderer.renderAnimation({
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

              return URL.createObjectURL(
                new Blob([bytes as BlobPart], { type: `image/${format}` }),
              );
            })()
          : renderer.renderAsDataUrl(node, {
              ...options,
              stylesheets: effectiveStylesheets,
              fetchedResources,
            });
        const duration = performance.now() - start;

        postMessage({
          type: "render-result",
          result: {
            status: "success",
            id: payload.id,
            outputUrl,
            duration,
            node,
            outputFormat: animationOptions?.format ?? options.format ?? "png",
            options,
          },
        });
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
    case "render-result": {
      throw new Error("Respond message should not be sent from main window.");
    }
    default: {
      payload satisfies never;
    }
  }
};
