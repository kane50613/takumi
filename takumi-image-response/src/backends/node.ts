import {
  type AnyNode,
  type ConstructRendererOptions,
  extractResourceUrls,
  Renderer,
  type RenderOptions,
} from "@takumi-rs/core";
import { fetchResources } from "@takumi-rs/helpers";
import { type FromJsxOptions, fromJsx } from "@takumi-rs/helpers/jsx";
import type { ReactNode } from "react";

let renderer: Renderer | undefined;

declare module "react" {
  interface DOMAttributes<T> {
    tw?: string;
  }
}

type ImageResponseOptionsWithRenderer = ResponseInit &
  RenderOptions & {
    renderer: Renderer;
    signal?: AbortSignal;
    jsx?: FromJsxOptions;
  };

type ImageResponseOptionsWithoutRenderer = ResponseInit &
  RenderOptions &
  ConstructRendererOptions & {
    signal?: AbortSignal;
    jsx?: FromJsxOptions;
  };

export type ImageResponseOptions =
  | ImageResponseOptionsWithRenderer
  | ImageResponseOptionsWithoutRenderer;

const defaultOptions = {
  format: "webp",
} as const satisfies ImageResponseOptions;

async function getRenderer(options?: ImageResponseOptions) {
  if (options && "renderer" in options) {
    return options.renderer;
  }

  if (!renderer) {
    renderer = new Renderer(options);

    return renderer;
  }

  const tasks: Promise<unknown>[] = [];

  const currentRenderer = renderer;
  if (!currentRenderer) {
    return renderer;
  }

  if (options?.fonts && options.fonts.length > 0) {
    tasks.push(currentRenderer.loadFonts(options.fonts));
  }

  if (options?.persistentImages) {
    tasks.push(
      ...options.persistentImages.map((image) =>
        currentRenderer.putPersistentImage(image.src, image.data),
      ),
    );
  }

  if (tasks.length > 0) {
    await Promise.all(tasks);
  }

  return currentRenderer;
}

function extractFetchedResources(
  node: AnyNode,
  options?: ImageResponseOptions,
) {
  if (options?.fetchedResources) {
    return options.fetchedResources;
  }

  const urls = extractResourceUrls(node);

  return fetchResources(urls);
}

function createStream(component: ReactNode, options?: ImageResponseOptions) {
  return new ReadableStream({
    type: "bytes",
    async start(controller) {
      try {
        const nodePromise = fromJsx(component, options?.jsx).then(
          async ({ node, stylesheets }) => {
            const fetchedResources = await extractFetchedResources(
              node,
              options,
            );
            return { node, fetchedResources, stylesheets };
          },
        );

        const [renderer, { node, fetchedResources, stylesheets }] =
          await Promise.all([getRenderer(options), nodePromise]);

        const mergedOptions = {
          width: options?.width,
          height: options?.height,
          format: options?.format,
          quality: options?.quality,
          dithering: options?.dithering,
          drawDebugBorder: options?.drawDebugBorder,
          devicePixelRatio: options?.devicePixelRatio,
          fetchedResources,
          stylesheets: options?.stylesheets ?? stylesheets,
        };

        const image = await renderer.render(
          node,
          mergedOptions,
          options?.signal,
        );

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
  WebP: "image/webp",
  Jpeg: "image/jpeg",
  Png: "image/png",
  raw: "application/octet-stream",
};

export class ImageResponse extends Response {
  constructor(component: ReactNode, options?: ImageResponseOptions) {
    const stream = createStream(component, options);
    const headers = new Headers(options?.headers);

    if (!headers.get("content-type")) {
      headers.set(
        "content-type",
        contentTypeMapping[options?.format ?? defaultOptions.format],
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
