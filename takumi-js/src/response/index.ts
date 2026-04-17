import type { ReactNode } from "react";
import { render, type RenderOptions } from "../render";
import type { ReactElementLike } from "@takumi-rs/helpers";

export type ImageResponseResult = Response & {
  readonly ready: Promise<void>;
};

export type ImageResponseOptions = RenderOptions &
  ResponseInit & {
    onError?: (error: unknown) => void | Promise<void>;
  };

export type ImageResponseFactory = (
  component: ReactNode,
  options?: ImageResponseOptions,
) => ImageResponseResult;

function mergeOptions(
  defaultOptions: ImageResponseOptions | undefined,
  options: ImageResponseOptions | undefined,
) {
  if (!defaultOptions) {
    return options;
  }

  if (!options) {
    return defaultOptions;
  }

  const headers = new Headers(defaultOptions?.headers);

  if (options?.headers) {
    const optionHeaders = new Headers(options.headers);

    optionHeaders.forEach((value, key) => {
      headers.set(key, value);
    });
  }

  return {
    ...defaultOptions,
    ...options,
    headers,
    stylesheets: [...(defaultOptions?.stylesheets ?? []), ...(options?.stylesheets ?? [])],
  };
}

const contentTypeMap: Record<NonNullable<RenderOptions["format"]>, string> = {
  png: "image/png",
  jpeg: "image/jpeg",
  webp: "image/webp",
  ico: "image/x-icon",
  raw: "application/octet-stream",
};

function defaultErrorHandler(error: unknown) {
  console.error("Failed to render image.");
  console.error(error);
}

/**
 * Creates a reusable ImageResponse factory with shared configuration.
 *
 * Use this to avoid repeating configuration like fonts, stylesheets, or cache headers
 * across multiple API routes.
 *
 * @example
 * ```tsx
 * const myImageResponse = createImageResponse({
 *   fonts: [{ name: "Inter", data: fontBuffer }],
 *   headers: { "Cache-Control": "public, max-age=31536000" }
 * });
 *
 * export function GET() {
 *   return myImageResponse(<div>Custom Og Image</div>);
 * }
 * ```
 *
 * @param defaultOptions - Options that will be applied to every response created by this factory.
 */
export function createImageResponse(defaultOptions?: ImageResponseOptions): ImageResponseFactory {
  return function imageResponse(
    element: ReactNode | ReactElementLike | string,
    options?: ImageResponseOptions,
  ) {
    const mergedOptions = mergeOptions(defaultOptions, options);

    let resolveReady: (value: void | PromiseLike<void>) => void;
    let rejectReady: (reason?: unknown) => void;
    const ready = new Promise<void>((resolve, reject) => {
      resolveReady = resolve;
      rejectReady = reject;
    });

    const stream = new ReadableStream({
      type: "bytes",
      async start(controller) {
        try {
          const image = await render(element, mergedOptions);
          controller.enqueue(image as ArrayBufferView<ArrayBuffer>);
          controller.close();
          resolveReady();
        } catch (error) {
          controller.error(error);

          rejectReady(error);
          const errorHandler = mergedOptions?.onError ?? defaultErrorHandler;

          await errorHandler(error);
        }
      },
    });
    const headers = new Headers(mergedOptions?.headers);

    if (!headers.get("content-type")) {
      headers.set("content-type", contentTypeMap[mergedOptions?.format ?? "png"]);
    }

    const response = new Response(stream, {
      headers,
      status: mergedOptions?.status,
      statusText: mergedOptions?.statusText,
    });

    return Object.defineProperty(response, "ready", {
      enumerable: false,
      value: ready,
      writable: false,
    }) as ImageResponseResult;
  };
}

let defaultImageResponse: ImageResponseFactory | undefined;

/**
 * A universal ImageResponse class for generating images in API routes.
 *
 * Drop-in compatible with `next/og`'s `ImageResponse`. It supports React elements,
 * custom fonts, Tailwind CSS (via `tw` prop), and various image formats.
 *
 * @example
 * ```tsx
 * import { ImageResponse } from "takumi-js/response";
 *
 * export function GET() {
 *   return new ImageResponse(
 *     <div tw="flex h-full w-full items-center justify-center bg-white">
 *       <h1 tw="text-6xl font-bold">Hello World</h1>
 *     </div>,
 *     { width: 1200, height: 630 }
 *   );
 * }
 * ```
 *
 * @param component - The JSX element to render.
 * @param options - Rendering and response options.
 */
export class ImageResponse extends Response {
  readonly ready: Promise<void>;

  constructor(component: ReactNode, options?: ImageResponseOptions) {
    defaultImageResponse ??= createImageResponse();

    const response = defaultImageResponse(component, options);

    super(response.body, response);
    this.ready = response.ready;
  }
}

export default ImageResponse;
