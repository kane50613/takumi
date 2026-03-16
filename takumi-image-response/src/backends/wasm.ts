import { fetchResources } from "@takumi-rs/helpers";
import { type EmojiType, extractEmojis } from "@takumi-rs/helpers/emoji";
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

declare module "react" {
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
  RenderOptions & {
    renderer: Renderer;
    jsx?: FromJsxOptions;
    emoji?: EmojiType | "from-font";
  };

type ImageResponseOptionsWithoutRenderer = ResponseInit &
  RenderOptions &
  ModuleOptions & {
    fonts?: Font[];
    persistentImages?: ImageSource[];
    jsx?: FromJsxOptions;
    emoji?: EmojiType | "from-font";
  };

export type ImageResponseOptions =
  | ImageResponseOptionsWithRenderer
  | ImageResponseOptionsWithoutRenderer;

function getRenderer(options?: ImageResponseOptions) {
  if (options && "renderer" in options) {
    return options.renderer;
  }

  if (!renderer) {
    renderer = new Renderer(options);

    return renderer;
  }

  if (options?.fonts) {
    for (const font of options.fonts) {
      renderer.loadFont(font);
    }
  }

  if (options?.persistentImages) {
    for (const image of options.persistentImages) {
      renderer.putPersistentImage(image);
    }
  }

  return renderer;
}

let initPromise: Promise<void> | null = null;

async function ensureWasmInitialized(options: ImageResponseOptions) {
  if (!("module" in options) || renderer) return;

  if (!initPromise) {
    initPromise = (async () => {
      let moduleResolved = await options.module;

      if (typeof moduleResolved === "object" && "default" in moduleResolved) {
        moduleResolved = moduleResolved.default;
      }

      await init({
        module_or_path: moduleResolved,
      });
    })();
  }

  await initPromise;
}

async function prepareNodeAndResources(
  component: ReactNode,
  options: ImageResponseOptions,
) {
  const localOptions = { ...options };
  let { node, stylesheets } = await fromJsx(component, localOptions.jsx);
  localOptions.stylesheets = [
    ...(localOptions.stylesheets ?? []),
    ...stylesheets,
  ];

  if (localOptions.emoji && localOptions.emoji !== "from-font") {
    node = extractEmojis(node, localOptions.emoji);
  }

  if (!localOptions.fetchedResources) {
    const urls = extractResourceUrls(node);
    if (urls.length > 0) {
      localOptions.fetchedResources = await fetchResources(urls);
    }
  }

  return { node, options: localOptions };
}

function createStream(component: ReactNode, options: ImageResponseOptions) {
  return new ReadableStream({
    type: "bytes",
    async start(controller) {
      try {
        await ensureWasmInitialized(options);

        const rendererInstance = getRenderer(options);
        const { node, options: localOptions } = await prepareNodeAndResources(
          component,
          options,
        );

        const image = rendererInstance.render(node, localOptions);

        controller.enqueue(image as Uint8Array<ArrayBuffer>);
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
  raw: "application/octet-stream",
};

const defaultOptions = {
  format: "webp",
} as const satisfies Partial<ImageResponseOptions>;

export class ImageResponse extends Response {
  constructor(component: ReactNode, options: ImageResponseOptions) {
    const stream = createStream(component, options);
    const headers = new Headers(options.headers);

    if (!headers.get("content-type")) {
      headers.set(
        "content-type",
        contentTypeMapping[options.format ?? defaultOptions.format],
      );
    }

    super(stream, {
      status: options.status,
      statusText: options.statusText,
      headers,
    });
  }
}

export default ImageResponse;
