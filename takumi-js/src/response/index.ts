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

/** Streams the rendered image; `ready` settles once the render finishes or fails. */
function renderStream(element: RenderInput, options?: ImageResponseOptions) {
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
        controller.enqueue(image);
        controller.close();
        resolveReady();
      } catch (error) {
        controller.error(error);

        rejectReady(error);
        const errorHandler = options?.onError ?? defaultErrorHandler;

        await errorHandler(error);
      }
    },
  });

  return { stream, ready };
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
    const { stream, ready } = renderStream(component, options);
    const headers = new Headers(options?.headers);

    if (!headers.get("content-type")) {
      headers.set("content-type", contentTypeMap[options?.format ?? "png"]);
    }

    super(stream, { headers, status: options?.status, statusText: options?.statusText });
    this.ready = ready;
  }
}

export default ImageResponse;
