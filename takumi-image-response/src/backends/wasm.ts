import { fromJsx } from "@takumi-rs/helpers/jsx";
import {
  type ByteBuf,
  collectNodeFetchTasks,
  type Font,
  Renderer,
  type RenderOptions,
} from "@takumi-rs/wasm";
import type { ReactNode } from "react";

let renderer: Renderer;

type PersistentImage = {
  src: string;
  data: ByteBuf;
};

const fontLoadMarker = new WeakSet<Font>();
const persistentImageLoadMarker = new WeakSet<PersistentImage>();

declare module "react" {
  // biome-ignore lint/correctness/noUnusedVariables: used for type inference
  interface DOMAttributes<T> {
    tw?: string;
  }
}

type ImageResponseOptionsWithRenderer = ResponseInit &
  RenderOptions & {
    renderer: Renderer;
  };

type ImageResponseOptionsWithoutRenderer = ResponseInit &
  RenderOptions & {
    fonts?: Font[];
    persistentImages?: PersistentImage[];
  };

export type ImageResponseOptions =
  | ImageResponseOptionsWithRenderer
  | ImageResponseOptionsWithoutRenderer;

function getRenderer(options?: ImageResponseOptions) {
  if (options && "renderer" in options) {
    return options.renderer;
  }

  renderer ??= new Renderer();

  if (options?.fonts) {
    for (const font of options.fonts) {
      loadFont(font, renderer);
    }
  }

  if (options?.persistentImages) {
    for (const image of options.persistentImages) {
      putPersistentImage(image, renderer);
    }
  }

  return renderer;
}

function loadFont(font: Font, renderer: Renderer) {
  if (fontLoadMarker.has(font)) return;

  renderer.loadFont(font);
}

function putPersistentImage(image: PersistentImage, renderer: Renderer) {
  if (persistentImageLoadMarker.has(image)) return;

  renderer.putPersistentImage(image.src, new Uint8Array(image.data));
}

const defaultOptions: ImageResponseOptions = {
  width: 1200,
  height: 630,
  format: "webp",
};

function createStream(
  component: ReactNode,
  options: ImageResponseOptions = { ...defaultOptions },
) {
  return new ReadableStream({
    async start(controller) {
      try {
        const renderer = getRenderer(options);

        const node = await fromJsx(component);

        if (!options.fetchedResources) {
          const urls = collectNodeFetchTasks(node);

          if (urls.length > 0) {
            options.fetchedResources = new Map(
              await Promise.all(
                urls.map(
                  async (url) =>
                    [
                      url,
                      await fetch(url).then((r) => r.arrayBuffer()),
                    ] as const,
                ),
              ),
            );
          }
        }

        const image = renderer.render(node, options);

        controller.enqueue(image);
        controller.close();
      } catch (error) {
        controller.error(error);
      }
    },
  });
}

const contentTypeMapping = {
  png: "image/png",
  jpeg: "image/jpeg",
  webp: "image/webp",
};

export class ImageResponse extends Response {
  constructor(component: ReactNode, options?: ImageResponseOptions) {
    const stream = createStream(component, options);
    const headers = new Headers(options?.headers);

    if (!headers.get("content-type")) {
      headers.set(
        "content-type",
        contentTypeMapping[options?.format ?? "webp"],
      );
    }

    super(stream, {
      status: options?.status,
      statusText: options?.statusText,
      headers,
    });
  }
}

export default ImageResponse;
