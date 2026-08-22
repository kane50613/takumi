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

/** How long a freed worker gets to answer the ping that follows a result. */
const LIVENESS_TIMEOUT_MS = 2_000;

/** Bounds what the worker can hand the page to hold, forged or not. */
const MAX_PREVIEW_BYTES = 4 * 1024 * 1024;

function previewSize(message: { html: string; cssContents?: string[] }) {
  return (
    message.html.length + (message.cssContents ?? []).reduce((sum, css) => sum + css.length, 0)
  );
}

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
    const restart = () => {
      setRenderError({
        status: "error",
        id: currentRequestIdRef.current,
        message: "the render never came back, so the worker was restarted",
      });
      setGeneration((current) => current + 1);
    };

    worker.onmessage = (event: MessageEvent) => {
      const message = messageSchema.parse(event.data);

      switch (message.type) {
        case "ready": {
          setIsReady(true);
          break;
        }
        case "pong": {
          if (message.id === currentRequestIdRef.current) clearTimeout(timeoutRef.current);
          break;
        }
        case "render-request":
        case "ping": {
          throw new Error("request is not possible for response");
        }
        case "preview-result": {
          if (
            message.id === currentRequestIdRef.current &&
            previewSize(message) <= MAX_PREVIEW_BYTES
          ) {
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

          // A result proves nothing about the worker: the evaluated code shares
          // its realm and can post one before spinning forever. The ping runs
          // on the event loop that code would be holding.
          worker.postMessage({ type: "ping", id: result.id } satisfies RenderMessageInput);
          clearTimeout(timeoutRef.current);
          timeoutRef.current = setTimeout(restart, LIVENESS_TIMEOUT_MS);

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
