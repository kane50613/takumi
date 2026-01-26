import { fetchResources } from "@takumi-rs/helpers";
import { type FromJsxOptions, fromJsx } from "@takumi-rs/helpers/jsx";
import init, {
  extractResourceUrls,
  type Font,
  type ImageSource,
  type InitInput,
  Renderer,
  type RenderOptions,
} from "@takumi-rs/wasm/no-bundler";
import type { ReactNode } from "react";

let renderer: Renderer;

const fontLoadMarker = new WeakSet<Font>();
const persistentImageLoadMarker = new WeakSet<ImageSource>();

declare module "react" {
  // biome-ignore lint/correctness/noUnusedVariables: used for type inference
  interface DOMAttributes<T> {
    tw?: string;
  }
}

type ModuleOptions = {
  /**
   * @description The WebAssembly module to use for the renderer.
   *
   * @example
   * For Cloudflare Workers, you can use the bundled WASM file.
   * ```ts
   * {
   *   module: import("@takumi-rs/wasm/takumi_wasm_bg.wasm"),
   * }
   * ```
   *
   * For Next.js Turbopack, you can use the nextjs helper.
   * ```ts
   * {
   *   module: import("@takumi-rs/wasm/next"),
   * }
   * ```
   *
   * For Vite, use `?url` suffix to get the URL of the WASM file.
   *
   * ```ts
   * {
   *   module: import("@takumi-rs/wasm/takumi_wasm_bg.wasm?url"),
   * }
   * ```
   */
  module: InitInput | Promise<InitInput> | { default: InitInput };
};

type ImageResponseOptionsWithRenderer = ResponseInit &
  RenderOptions &
  ModuleOptions & {
    renderer: Renderer;
    jsx?: FromJsxOptions;
  };

type ImageResponseOptionsWithoutRenderer = ResponseInit &
  RenderOptions &
  ModuleOptions & {
    fonts?: Font[];
    persistentImages?: ImageSource[];
    jsx?: FromJsxOptions;
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

function putPersistentImage(image: ImageSource, renderer: Renderer) {
  if (persistentImageLoadMarker.has(image)) return;

  renderer.putPersistentImage(image);
}

function createStream(component: ReactNode, options: ImageResponseOptions) {
  options.format ??= "webp";

  return new ReadableStream({
    async start(controller) {
      try {
        let moduleResolved = await options.module;

        if (typeof moduleResolved === "object" && "default" in moduleResolved) {
          moduleResolved = moduleResolved.default;
        }

        await init({
          module_or_path: moduleResolved,
        });

        const renderer = getRenderer(options);

        const node = await fromJsx(component, options.jsx);

        if (!options.fetchedResources) {
          const urls = extractResourceUrls(node);

          if (urls.length > 0) {
            options.fetchedResources = await fetchResources(urls);
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
  constructor(component: ReactNode, options: ImageResponseOptions) {
    options.format ??= "webp";

    const stream = createStream(component, options);
    const headers = new Headers(options.headers);

    if (!headers.get("content-type")) {
      headers.set("content-type", contentTypeMapping[options.format]);
    }

    super(stream, {
      status: options.status,
      statusText: options.statusText,
      headers,
    });
  }
}

export default ImageResponse;
