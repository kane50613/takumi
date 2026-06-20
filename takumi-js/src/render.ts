import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import { extractEmojis, type EmojiType } from "@takumi-rs/helpers/emoji";
import { extractResourceUrls, fetchResources } from "@takumi-rs/helpers";
import { fromJsx, type FromJsxOptions } from "@takumi-rs/helpers/jsx";
import { getImports } from "./import";
import type { ReactNode } from "react";
import type { FetchResourcesOptions, Node, ReactElementLike } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";

type InnerRenderOptions = napi.RenderOptions | wasm.RenderOptions;

type RenderOptionsWithRenderer = InnerRenderOptions & {
  renderer: napi.Renderer | wasm.Renderer;
  signal?: AbortSignal;
  jsx?: FromJsxOptions;
  resourcesOptions?: FetchResourcesOptions;
  /**
   * @description The emoji provider to use when rendering emojis. If set to `"from-font"`, the renderer will attempt to source emoji glyphs from the loaded fonts.
   * @default "twemoji"
   */
  emoji?: EmojiType | "from-font";
};

export type ManagedRendererOptions = {
  /**
   * @description The WebAssembly module to use for the renderer. If not provided, the default resolving strategy will be used.
   */
  module?: wasm.InitInput | Promise<wasm.InitInput> | { default: wasm.InitInput };
};

export type RenderOptionsWithoutRenderer = Omit<RenderOptionsWithRenderer, "renderer"> &
  ManagedRendererOptions;

export type RenderOptions = RenderOptionsWithRenderer | RenderOptionsWithoutRenderer;

let globalRenderer: napi.Renderer | wasm.Renderer | undefined;

export type RenderInput = ReactNode | ReactElementLike | Node | string;

function isTakumiNode(element: unknown): element is Node {
  if (typeof element !== "object" || element === null || !("type" in element)) {
    return false;
  }

  return element.type === "container" || element.type === "text" || element.type === "image";
}

async function transformElement(element: RenderInput, options?: RenderOptions) {
  if (isTakumiNode(element)) {
    return {
      node: element,
      stylesheets: [],
    };
  }

  if (typeof element === "string") {
    return fromHtml(element);
  }

  return fromJsx(element, options?.jsx);
}

/**
 * Renders a React element, HTML string, or Takumi node tree into an image.
 *
 * This function automatically detects the best renderer for your environment (native Rust on Node.js,
 * WASM on Edge/Workers) and handles resource fetching (fonts, images) and emoji extraction.
 *
 * @example
 * ```tsx
 * import { render } from "takumi-js";
 *
 * const buffer = await render(
 *   <div tw="bg-blue-500 text-white p-4">Hello World</div>,
 *   { width: 1200, height: 630 }
 * );
 * ```
 *
 * @param element - The content to render. Can be a JSX element (React-like), an HTML string, or a pre-constructed node tree.
 * @param options - Configuration for rendering, including dimensions, format, fonts, and more.
 * @returns A promise that resolves to the rendered image data (Buffer/Uint8Array).
 */
export async function render(element: RenderInput, options?: RenderOptions) {
  options?.signal?.throwIfAborted();

  const imports = await getImports(options && "module" in options ? options.module : undefined);
  const isExternalRenderer = options && "renderer" in options;
  const renderer = isExternalRenderer
    ? options.renderer
    : (globalRenderer ??= new imports.Renderer());

  const { node: originalNode, stylesheets } = await transformElement(element, options);
  const emojiType = options?.emoji ?? "twemoji";

  const node = emojiType !== "from-font" ? extractEmojis(originalNode, emojiType) : originalNode;

  const providedImages = new Map<string, napi.ImageLoader | wasm.ImageLoader>();

  for (const image of options?.images ?? []) {
    providedImages.set(image.src, image);
  }

  const fetched = await fetchResources(
    extractResourceUrls(node).filter((url) => !providedImages.has(url)),
    options?.resourcesOptions,
  );

  const images = [...providedImages.values(), ...fetched];

  // The WASM renderer is synchronous and ignores the signal argument, so honor an
  // abort that happened during the async font/resource loading before the blocking call.
  options?.signal?.throwIfAborted();

  return renderer.render(node, {
    ...options,
    images,
    stylesheets: [...(options?.stylesheets ?? []), ...stylesheets],
  });
}
