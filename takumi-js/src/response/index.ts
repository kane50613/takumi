import type { RenderReport } from "@takumi-rs/core";
import { type RenderInput, type RenderOptions, renderWithReport } from "../render";

export type ImageResponseResult = Response & {
  readonly ready: Promise<RenderReport>;
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

/** Streams the rendered image; `ready` resolves to the render's report once it finishes, or rejects when it fails. */
function renderStream(element: RenderInput, options?: ImageResponseOptions) {
  let resolveReady: (report: RenderReport) => void;
  let rejectReady: (reason?: unknown) => void;
  const ready = new Promise<RenderReport>((resolve, reject) => {
    resolveReady = resolve;
    rejectReady = reject;
  });

  // A rejection is also surfaced via controller.error; without this, callers
  // that never await `ready` crash the process with an unhandledRejection.
  ready.catch(() => {});

  const stream = new ReadableStream({
    async start(controller) {
      try {
        const { image, report } = await renderWithReport(element, options);
        controller.enqueue(image);
        controller.close();
        resolveReady(report);
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
 * A `Response` with a rendered image, compatible with `next/og`'s `ImageResponse`.
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
 */
export class ImageResponse extends Response {
  /** Resolves to the report of what the render could not draw once it finishes, and rejects when it fails, as under `strict` for any warning. */
  readonly ready: Promise<RenderReport>;

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
