import type { ReactNode } from "react";
import { render, type RenderOptions } from "../render";

const defaultFormat = "webp";

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
  raw: "application/octet-stream",
};

function defaultErrorHandler(error: unknown) {
  console.error("Failed to render image.");
  console.error(error);
}

export function createImageResponse(defaultOptions?: ImageResponseOptions): ImageResponseFactory {
  return function imageResponse(element: ReactNode, options?: ImageResponseOptions) {
    const mergedOptions: ImageResponseOptions = {
      ...mergeOptions(defaultOptions, options),
      format: options?.format ?? defaultOptions?.format ?? defaultFormat,
    };
    const {
      promise: ready,
      reject: rejectReady,
      resolve: resolveReady,
    } = Promise.withResolvers<void>();

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
      headers.set("content-type", contentTypeMap[mergedOptions.format ?? defaultFormat]);
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
