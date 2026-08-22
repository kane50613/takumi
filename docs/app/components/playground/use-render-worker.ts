import { useEffect, useRef, useState } from "react";
import type { z } from "zod/mini";
import {
  messageSchema,
  type RenderMessageInput,
  type renderResultSchema,
} from "~/playground/schema";
import TakumiWorker from "~/playground/worker?worker";

export type RenderResult = z.infer<typeof renderResultSchema>["result"];
export type RenderSuccess = Extract<RenderResult, { status: "success" }> & { outputSize: number };
export type RenderError = Extract<RenderResult, { status: "error" }>;

export type BrowserPreviewData = {
  html: string;
  width?: number;
  height?: number;
  padding?: string;
  cssContents?: string[];
};

function isBlobUrl(url: string | undefined): url is string {
  return typeof url === "string" && url.startsWith("blob:");
}

function mimeType(result: RenderResult & { status: "success" }) {
  return result.outputKind === "pdf" ? "application/pdf" : `image/${result.outputFormat}`;
}

/** A render that outlives this has hit a loop the worker cannot leave on its own. */
const RENDER_TIMEOUT_MS = 15_000;

export function useRenderWorker(ranCode: string | undefined) {
  const [isReady, setIsReady] = useState(false);
  const [lastSuccess, setLastSuccess] = useState<RenderSuccess>();
  const [renderError, setRenderError] = useState<RenderError>();
  const [browserPreview, setBrowserPreview] = useState<BrowserPreviewData>();
  const [generation, setGeneration] = useState(0);
  const currentRequestIdRef = useRef(0);
  const workerRef = useRef<Worker | undefined>(undefined);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    const worker = new TakumiWorker();

    worker.onmessage = (event: MessageEvent) => {
      const message = messageSchema.parse(event.data);

      switch (message.type) {
        case "ready": {
          setIsReady(true);
          break;
        }
        case "render-request": {
          throw new Error("request is not possible for response");
        }
        case "preview-result": {
          if (message.id === currentRequestIdRef.current) {
            setBrowserPreview({
              html: message.html,
              width: message.width,
              height: message.height,
              padding: message.padding,
              cssContents: message.cssContents,
            });
          }
          break;
        }
        case "render-result": {
          const { result } = message;
          if (result.id !== currentRequestIdRef.current) break;

          clearTimeout(timeoutRef.current);

          if (result.status === "success") {
            const blob = new Blob([result.outputBuffer as BlobPart], { type: mimeType(result) });
            setLastSuccess({
              ...result,
              outputUrl: URL.createObjectURL(blob),
              outputSize: blob.size,
            });
            setRenderError(undefined);
          } else {
            setRenderError(result);
          }
          break;
        }
        default: {
          message satisfies never;
        }
      }
    };

    workerRef.current = worker;

    return () => {
      worker.terminate();
      workerRef.current = undefined;
      setIsReady(false);
    };
  }, [generation]);

  useEffect(() => {
    if (!isReady || ranCode === undefined) return;

    const requestId = currentRequestIdRef.current + 1;
    currentRequestIdRef.current = requestId;
    workerRef.current?.postMessage({
      type: "render-request",
      id: requestId,
      code: ranCode,
    } satisfies RenderMessageInput);

    timeoutRef.current = setTimeout(() => {
      setRenderError({
        status: "error",
        id: requestId,
        message: `the render ran past ${RENDER_TIMEOUT_MS / 1000}s, so the worker was restarted`,
      });
      setGeneration((current) => current + 1);
    }, RENDER_TIMEOUT_MS);

    return () => clearTimeout(timeoutRef.current);
  }, [isReady, ranCode]);

  useEffect(() => {
    if (!isBlobUrl(lastSuccess?.outputUrl)) return;

    const url = lastSuccess.outputUrl;
    return () => URL.revokeObjectURL(url);
  }, [lastSuccess]);

  return { isReady, lastSuccess, renderError, browserPreview };
}
