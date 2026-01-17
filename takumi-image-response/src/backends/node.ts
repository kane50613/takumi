import {
  type ConstructRendererOptions,
  collectNodeFetchTasks,
  type Font,
  type PersistentImage,
  Renderer,
  type RenderOptions,
} from "@takumi-rs/core";
import { fetchResources } from "@takumi-rs/helpers";
import { type FromJsxOptions, fromJsx } from "@takumi-rs/helpers/jsx";
import type { ReactNode } from "react";

let renderer: Renderer | undefined;

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

    if (options?.fonts) {
      for (const font of options.fonts) {
        fontLoadMarker.add(font);
      }
    }

    if (options?.persistentImages) {
      for (const image of options.persistentImages) {
        persistentImageLoadMarker.add(image);
      }
    }

    return renderer;
  }

  await loadOptions(renderer, options);

  return renderer;
}

async function loadOptions(
  renderer: Renderer,
  options?: ImageResponseOptionsWithoutRenderer,
) {
  await loadFonts(renderer, options?.fonts ?? []);

  if (options?.persistentImages) {
    for (const image of options.persistentImages) {
      await putPersistentImage(renderer, image);
    }
  }
}

function loadFonts(renderer: Renderer, fonts: Font[]) {
  const fontsToLoad = fonts.filter((font) => !fontLoadMarker.has(font));

  for (const font of fontsToLoad) {
    fontLoadMarker.add(font);
  }

  return renderer.loadFonts(fontsToLoad);
}

function putPersistentImage(renderer: Renderer, image: PersistentImage) {
  if (persistentImageLoadMarker.has(image)) {
    return;
  }

  persistentImageLoadMarker.add(image);

  return renderer.putPersistentImage(image.src, image.data);
}

function createStream(component: ReactNode, options: ImageResponseOptions) {
  return new ReadableStream({
    async start(controller) {
      try {
        const renderer = await getRenderer(options);

        const node = await fromJsx(component, options?.jsx);

        if (!options.fetchedResources) {
          const urls = collectNodeFetchTasks(node);

          if (urls.length > 0) {
            options.fetchedResources = await fetchResources(urls);
          }
        }

        const image = await renderer.render(
          node,
          options ?? defaultOptions,
          options?.signal,
        );

        controller.enqueue(image);
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
    const mergedOptions: ImageResponseOptions = {
      ...defaultOptions,
      ...options,
    };

    const stream = createStream(component, mergedOptions);
    const headers = new Headers(mergedOptions.headers);

    if (!headers.get("content-type")) {
      headers.set(
        "content-type",
        contentTypeMapping[mergedOptions.format ?? defaultOptions.format],
      );
    }

    super(stream, {
      status: mergedOptions.status,
      statusText: mergedOptions.statusText,
      headers,
    });
  }
}

export default ImageResponse;
