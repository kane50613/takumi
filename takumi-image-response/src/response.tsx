import { fetchResources } from "@takumi-rs/helpers";
import { type EmojiType, extractEmojis } from "@takumi-rs/helpers/emoji";
import { type FromJsxOptions, fromJsx } from "@takumi-rs/helpers/jsx";
import type { ReactNode } from "react";
import type * as napi from "@takumi-rs/core";
import type * as wasm from "@takumi-rs/wasm";
import { getImports, type Imports } from "./import";

let renderer: napi.Renderer | wasm.Renderer | undefined;

const fontMarks = new WeakSet<napi.Font | wasm.Font>();
const imageMarks = new WeakSet<napi.ImageSource | wasm.ImageSource>();

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}

type RenderOptions = napi.RenderOptions | wasm.RenderOptions;
type ConstructRendererOptions =
  | napi.ConstructRendererOptions
  | (wasm.ConstructRendererOptions & {
      /**
       * @description The WebAssembly module to use for the renderer. If not provided, the default resolving strategy will be used.
       */
      module?: wasm.InitInput | Promise<wasm.InitInput> | { default: wasm.InitInput };
    });

type ImageResponseOptionsWithRenderer = ResponseInit &
  RenderOptions & {
    renderer: napi.Renderer | wasm.Renderer;
    signal?: AbortSignal;
    jsx?: FromJsxOptions;
    emoji?: EmojiType | "from-font";
  };

type ImageResponseOptionsWithoutRenderer = Omit<ImageResponseOptionsWithRenderer, "renderer"> &
  ConstructRendererOptions;

export type ImageResponseOptions =
  | ImageResponseOptionsWithRenderer
  | ImageResponseOptionsWithoutRenderer;

const defaultOptions = {
  format: "webp",
} as const satisfies ImageResponseOptions;

async function getRenderer(options: ImageResponseOptions | undefined, imports: Imports) {
  if (options && "renderer" in options) {
    return options.renderer;
  }

  if (!renderer) {
    renderer = new imports.Renderer(options);

    return renderer;
  }

  const tasks: Promise<unknown>[] = [];

  if (options?.fonts && options.fonts.length > 0) {
    if ("loadFonts" in renderer) {
      const filteredFonts = options.fonts.filter((font) => {
        if (fontMarks.has(font)) {
          return false;
        }

        fontMarks.add(font);
        return true;
      });

      tasks.push(renderer.loadFonts(filteredFonts));
    } else {
      for (const font of options.fonts) {
        if (fontMarks.has(font)) {
          continue;
        }

        fontMarks.add(font);

        renderer.loadFont(font);
      }
    }
  }

  if (options?.persistentImages) {
    for (const image of options.persistentImages) {
      if (imageMarks.has(image)) {
        continue;
      }

      imageMarks.add(image);

      const maybePromise = renderer.putPersistentImage(image, options.signal);

      if (maybePromise instanceof Promise) {
        tasks.push(maybePromise);
      }
    }
  }

  if (tasks.length > 0) {
    await Promise.all(tasks);
  }

  return renderer;
}

async function extractFetchedResources(
  node: napi.Node | wasm.Node,
  options: ImageResponseOptions | undefined,
  imports: Imports,
) {
  if (options?.fetchedResources) {
    return options.fetchedResources;
  }

  const urls = imports.extractResourceUrls(node);

  return fetchResources(urls);
}

function createStream(component: ReactNode, options?: ImageResponseOptions) {
  return new ReadableStream({
    type: "bytes",
    async start(controller) {
      try {
        const imports = await getImports(
          options !== undefined && "module" in options ? options.module : undefined,
        );
        const nodePromise = fromJsx(component, options?.jsx).then(async ({ node, stylesheets }) => {
          if (options?.emoji && options.emoji !== "from-font") {
            node = extractEmojis(node, options.emoji);
          }

          const fetchedResources = await extractFetchedResources(node, options, imports);

          return { node, fetchedResources, stylesheets };
        });

        const [renderer, { node, fetchedResources, stylesheets }] = await Promise.all([
          getRenderer(options, imports),
          nodePromise,
        ]);

        const mergedOptions = {
          ...options,
          fetchedResources,
          stylesheets: [...(options?.stylesheets ?? []), ...stylesheets],
        };

        const image = await renderer.render(node, mergedOptions, options?.signal);

        controller.enqueue(image as ArrayBufferView<ArrayBuffer>);
        controller.close();
      } catch (error) {
        controller.error(error);
      }
    },
  });
}

const contentTypeMapping = {
  webp: "image/webp",
  png: "image/png",
  jpeg: "image/jpeg",
  raw: "application/octet-stream",
};

export class ImageResponse extends Response {
  constructor(component: ReactNode, options?: ImageResponseOptions) {
    const stream = createStream(component, options);
    const headers = new Headers(options?.headers);

    if (!headers.get("content-type")) {
      headers.set("content-type", contentTypeMapping[options?.format ?? defaultOptions.format]);
    }

    super(stream, {
      status: options?.status,
      statusText: options?.statusText,
      headers,
    });
  }
}

export default ImageResponse;
