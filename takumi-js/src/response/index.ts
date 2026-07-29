import { render, type RenderInput, type RenderOptions } from "../render";

export type ImageResponseResult = Response & {
  readonly ready: Promise<void>;
};

export type ImageResponseOptions = RenderOptions &
  ResponseInit & {
    onError?: (error: unknown) => void | Promise<void>;
  };

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

async function notifyError(error: unknown, options?: ImageResponseOptions) {
  try {
    await (options?.onError ?? defaultErrorHandler)(error);
  } catch (handlerError) {
    console.error("The onError handler failed.");
    console.error(handlerError);
  }
}

function responseHeaders(options?: ImageResponseOptions) {
  const headers = new Headers(options?.headers);

  if (!headers.get("content-type")) {
    headers.set("content-type", contentTypeMap[options?.format ?? "png"]);
  }

  return headers;
}

// Web Crypto and `Response` reject a view that could be backed by a SharedArrayBuffer,
// which no renderer returns; the copy is a fallback the type system asks for.
function arrayBufferView(image: Uint8Array): Uint8Array<ArrayBuffer> {
  return image.buffer instanceof ArrayBuffer
    ? new Uint8Array(image.buffer, image.byteOffset, image.byteLength)
    : new Uint8Array(image);
}

async function strongEtag(image: Uint8Array<ArrayBuffer>) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", image));
  let hex = "";

  for (const byte of digest) {
    hex += byte.toString(16).padStart(2, "0");
  }

  return `"${hex}"`;
}

function buildImageResponse(
  element: RenderInput,
  options?: ImageResponseOptions,
): ImageResponseResult {
  let resolveReady: (value: void | PromiseLike<void>) => void;
  let rejectReady: (reason?: unknown) => void;
  const ready = new Promise<void>((resolve, reject) => {
    resolveReady = resolve;
    rejectReady = reject;
  });

  // A rejection is also surfaced via controller.error; without this, callers
  // that never await `ready` crash the process with an unhandledRejection.
  ready.catch(() => {});

  const stream = new ReadableStream({
    async start(controller) {
      try {
        const image = await render(element, options);
        controller.enqueue(arrayBufferView(image));
        controller.close();
        resolveReady();
      } catch (error) {
        controller.error(error);

        rejectReady(error);
        await notifyError(error, options);
      }
    },
  });

  const response = new Response(stream, {
    headers: responseHeaders(options),
    status: options?.status,
    statusText: options?.statusText,
  });

  return Object.defineProperty(response, "ready", {
    enumerable: false,
    value: ready,
    writable: false,
  }) as ImageResponseResult;
}

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

  constructor(component: RenderInput, options?: ImageResponseOptions) {
    const response = buildImageResponse(component, options);

    super(response.body, response);
    this.ready = response.ready;
  }

  /**
   * The awaited counterpart of the constructor: it renders first, then hands back a
   * `Response` whose body is the finished bytes instead of a stream. That costs an `await`
   * at the call site and buys a known body length, no stream to pump, and a strong `ETag`
   * derived from the bytes, which the constructor cannot set because its headers are read
   * while the render is still in flight.
   *
   * Nothing here inspects `If-None-Match`. The `ETag` is what lets a server, adapter, or
   * CDN answer a conditional request with `304 Not Modified`.
   *
   * An `etag` passed through `headers` is left alone. Rejects with the render error, after
   * `onError` runs.
   *
   * The digest comes from global Web Crypto, which Node exposes from 19 onwards (18 needs
   * `--experimental-global-webcrypto`).
   *
   * @example
   * ```tsx
   * import { ImageResponse } from "takumi-js/response";
   *
   * export function GET() {
   *   return ImageResponse.render(<OgImage />, { width: 1200, height: 630 });
   * }
   * ```
   *
   * @param component - The JSX element to render.
   * @param options - Rendering and response options.
   */
  static async render(component: RenderInput, options?: ImageResponseOptions): Promise<Response> {
    let image: Uint8Array<ArrayBuffer>;

    try {
      image = arrayBufferView(await render(component, options));
    } catch (error) {
      await notifyError(error, options);

      throw error;
    }

    const headers = responseHeaders(options);

    if (!headers.has("etag")) {
      headers.set("etag", await strongEtag(image));
    }

    return new Response(image, {
      headers,
      status: options?.status,
      statusText: options?.statusText,
    });
  }
}

export default ImageResponse;
