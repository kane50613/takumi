import type { Node } from "takumi-js/helpers";
import wasm, { init, Renderer } from "takumi-js/wasm";

export type LiveCardRequest =
  | { type: "setup"; node: Node; headline: string; width: number; height: number }
  | { type: "render"; id: number; headline: string };

export type LiveCardResponse =
  | { type: "result"; id: number; image: Uint8Array<ArrayBuffer>; duration: number }
  | { type: "error"; id: number; message: string };

let card: Extract<LiveCardRequest, { type: "setup" }> | undefined;

const renderer = init({ module_or_path: wasm }).then(() => new Renderer());

/** `node` with every text equal to `from` replaced by `to`. */
function replaceText(node: Node, from: string, to: string): Node {
  if (node.type === "text") {
    return node.text === from ? { ...node, text: to } : node;
  }

  if (node.type === "container" && node.children) {
    return { ...node, children: node.children.map((child) => replaceText(child, from, to)) };
  }

  return node;
}

self.onmessage = async ({ data }: MessageEvent<LiveCardRequest>) => {
  if (data.type === "setup") {
    card = data;
    return;
  }

  if (!card) {
    return;
  }

  try {
    const ready = await renderer;
    const start = performance.now();
    const image = await ready.render(replaceText(card.node, card.headline, data.headline), {
      width: card.width,
      height: card.height,
      format: "webp",
      quality: 90,
    });
    const response: LiveCardResponse = {
      type: "result",
      id: data.id,
      image,
      duration: performance.now() - start,
    };

    self.postMessage(response, { transfer: [image.buffer] });
  } catch (error) {
    const response: LiveCardResponse = {
      type: "error",
      id: data.id,
      message: error instanceof Error ? error.message : String(error),
    };

    self.postMessage(response);
  }
};
