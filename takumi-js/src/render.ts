import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import { extractEmojis, type EmojiType } from "@takumi-rs/helpers/emoji";
import { fromJsx, type FromJsxOptions } from "@takumi-rs/helpers/jsx";
import { loadRendererResources, type ManagedRendererOptions } from "./renderer";
import { getImports } from "./import";
import type { ReactNode } from "react";
import { fetchResources, type ReactElementLike } from "@takumi-rs/helpers";

type InnerRenderOptions = napi.RenderOptions | wasm.RenderOptions;

type RenderOptionsWithRenderer = InnerRenderOptions & {
  renderer: napi.Renderer | wasm.Renderer;
  signal?: AbortSignal;
  jsx?: FromJsxOptions;
  /**
   * @description The emoji provider to use when rendering emojis. If set to `"from-font"`, the renderer will attempt to source emoji glyphs from the loaded fonts.
   * @default "twemoji"
   */
  emoji?: EmojiType | "from-font";
};

export type RenderOptionsWithoutRenderer = Omit<RenderOptionsWithRenderer, "renderer"> &
  ManagedRendererOptions;

export type RenderOptions = RenderOptionsWithRenderer | RenderOptionsWithoutRenderer;

let globalRenderer: napi.Renderer | wasm.Renderer | undefined;

export async function render(element: ReactNode | ReactElementLike, options?: RenderOptions) {
  const imports = await getImports(options && "module" in options ? options.module : undefined);
  const isExternalRenderer = options && "renderer" in options;
  const renderer = isExternalRenderer
    ? options.renderer
    : (globalRenderer ??= new imports.Renderer({
        loadDefaultFonts: options?.loadDefaultFonts,
      }));

  if (!isExternalRenderer) {
    await loadRendererResources(renderer, options);
  }

  const { node: originalNode, stylesheets } = await fromJsx(element, options?.jsx);
  const emojiType = options?.emoji ?? "twemoji";

  const node = emojiType !== "from-font" ? extractEmojis(originalNode, emojiType) : originalNode;
  const fetchedResources =
    options?.fetchedResources !== undefined
      ? options.fetchedResources
      : await fetchResources(imports.extractResourceUrls(node));

  const renderOptions = {
    ...options,
    fetchedResources,
    stylesheets: [...(options?.stylesheets ?? []), ...stylesheets],
  };

  return renderer.render(node, renderOptions, options?.signal);
}

export async function extractResourceUrls(element: ReactNode | ReactElementLike) {
  const imports = await getImports();
  const { node } = await fromJsx(element);

  return imports.extractResourceUrls(node);
}
