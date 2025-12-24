import { fromJsx } from "@takumi-rs/helpers/jsx";
import initWasm, { collectNodeFetchTasks, Renderer } from "@takumi-rs/wasm";
import wasmUrl from "@takumi-rs/wasm/takumi_wasm_bg.wasm?url";
import * as React from "react";
import { transform } from "sucrase";
import * as z from "zod/mini";
import {
  messageSchema,
  optionsSchema,
  type RenderMessageInput,
} from "./schema";

function postMessage(message: RenderMessageInput) {
  return self.postMessage(message);
}

const exportsSchema = z.object({
  default: z.function(),
  options: optionsSchema,
});

let renderer: Renderer | undefined;

// Cache for fetched resources to avoid repeated network requests
const fetchCache = new Map<string, ArrayBuffer>();

async function cachedFetch(url: string): Promise<ArrayBuffer> {
  const cached = fetchCache.get(url);
  if (cached !== undefined) {
    return cached;
  }

  const response = await fetch(url);
  const buffer = await response.arrayBuffer();

  fetchCache.set(url, buffer);

  return buffer;
}

(async () => {
  const [_, normalFont, monoFont, emojiFont] = await Promise.all([
    initWasm({ module_or_path: wasmUrl }),
    fetch("/fonts/Geist.woff2").then((r) => r.arrayBuffer()),
    fetch("/fonts/GeistMono.woff2").then((r) => r.arrayBuffer()),
    fetch("/fonts/TwemojiMozilla-colr.woff2").then((r) => r.arrayBuffer()),
  ]);

  renderer = new Renderer();
  renderer.loadFont({
    data: normalFont,
    name: "Geist",
  });
  renderer.loadFont({
    data: monoFont,
    name: "Geist Mono",
  });
  renderer.loadFont({
    data: emojiFont,
    name: "Twemoji Mozilla",
  });

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
        const { default: component, options } = evaluateCodeExports(
          payload.code,
        );
        const node = await fromJsx(
          React.createElement(
            component as React.JSXElementConstructor<unknown>,
          ),
        );

        const resourceUrls = collectNodeFetchTasks(node);

        const fetchedResources = new Map(
          await Promise.all(
            resourceUrls.map(
              async (url) => [url, await cachedFetch(url)] as const,
            ),
          ),
        );

        const start = performance.now();
        const dataUrl = renderer.renderAsDataUrl(node, {
          ...options,
          fetchedResources,
        });
        const duration = performance.now() - start;

        postMessage({
          type: "render-result",
          result: {
            status: "success",
            id: payload.id,
            dataUrl,
            duration,
            node,
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
