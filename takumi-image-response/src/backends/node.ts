import {
  type ConstructRendererOptions,
  type Font,
  type PersistentImage,
  Renderer,
  type RenderOptions,
} from "@takumi-rs/core";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import type { ReactNode } from "react";

let renderer: Renderer | undefined;

const fontLoadMarker = new WeakSet<Font>();
const persistentImageLoadMarker = new WeakSet<PersistentImage>();

type ImageResponseOptionsWithRenderer = ResponseInit &
  RenderOptions & {
    renderer: Renderer;
    signal?: AbortSignal;
  };

type ImageResponseOptionsWithoutRenderer = ResponseInit &
  RenderOptions &
  ConstructRendererOptions & {
    signal?: AbortSignal;
  };

export type ImageResponseOptions =
  | ImageResponseOptionsWithRenderer
  | ImageResponseOptionsWithoutRenderer;

const defaultOptions: ImageResponseOptions = {
  width: 1200,
  height: 630,
  format: "webp",
};

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

function createStream(component: ReactNode, options?: ImageResponseOptions) {
  return new ReadableStream({
    async start(controller) {
      try {
        const renderer = await getRenderer(options);

        const node = await fromJsx(component);
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
  avif: "image/avif",
  png: "image/png",
  jpeg: "image/jpeg",
  WebP: "image/webp",
  Jpeg: "image/jpeg",
  Png: "image/png",
  Avif: "image/avif",
  raw: "application/octet-stream",
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
