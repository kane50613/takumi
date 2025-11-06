import { useEffect, useRef, useState } from "react";
import type { PanelGroupProps } from "react-resizable-panels";
import { useSearchParams } from "react-router";
import type { z } from "zod/mini";
import defaultTemplate from "~/playground/default?raw";
import {
  messageSchema,
  type RenderMessageInput,
  type renderResultSchema,
} from "~/playground/schema";
import { compressCode, decompressCode } from "~/playground/share";
import TakumiWorker from "~/playground/worker?worker";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../ui/resizable";
import { ComponentEditor } from "./component-editor";

const mobileViewportWidth = 640;

function useDirection() {
  const [direction, setDirection] =
    useState<PanelGroupProps["direction"]>("horizontal");

  const resize = () => {
    setDirection(
      window.innerWidth < mobileViewportWidth ? "vertical" : "horizontal",
    );
  };

  useEffect(() => {
    resize();

    addEventListener("resize", resize);

    return () => removeEventListener("resize", resize);
  });

  return direction;
}

export default function Playground() {
  const [code, setCode] = useState<string>();
  const [rendered, setRendered] =
    useState<z.infer<typeof renderResultSchema>["result"]>();
  const [isReady, setIsReady] = useState(false);
  const currentRequestIdRef = useRef(0);

  const workerRef = useRef<Worker | undefined>(undefined);
  const direction = useDirection();
  const [searchParams, setSearchParams] = useSearchParams();

  const codeQuery = searchParams.get("code");

  useEffect(() => {
    if (codeQuery) decompressCode(codeQuery).then(setCode);
    else setCode(defaultTemplate);
  }, [codeQuery]);

  useEffect(() => {
    if (!code) return;

    if (code === defaultTemplate) {
      return setSearchParams((prev) => {
        prev.delete("code");

        return prev;
      });
    }

    compressCode(code).then((base64) => {
      setSearchParams((prev) => {
        prev.set("code", base64);

        return prev;
      });
    });
  }, [code, setSearchParams]);

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
        case "render-result": {
          // Only update if this result matches the current request
          if (message.result.id === currentRequestIdRef.current) {
            setRendered(message.result);
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
  }, []);

  useEffect(() => {
    if (isReady && code) {
      const requestId = currentRequestIdRef.current + 1;
      currentRequestIdRef.current = requestId;
      workerRef.current?.postMessage({
        type: "render-request",
        id: requestId,
        code,
      } satisfies RenderMessageInput);
    }
  }, [isReady, code]);

  return (
    <div className="h-[calc(100dvh-3.5rem)]">
      <ResizablePanelGroup direction={direction}>
        <ResizablePanel defaultSize={50}>
          {code && <ComponentEditor code={code} setCode={setCode} />}
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel defaultSize={50}>
          {rendered && <RenderPreview result={rendered} />}
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}

function RenderPreview({
  result,
}: {
  result: z.infer<typeof renderResultSchema>["result"];
}) {
  if (result.status === "error") {
    return (
      <div className="h-full w-full flex justify-center items-center flex-col bg-destructive/30 text-destructive-foreground">
        <p className="text-xl font-medium">Error</p>
        <span className="opacity-80">{result.message}</span>
      </div>
    );
  }

  return (
    <div className="h-full w-full flex justify-center gap-2 flex-col">
      <img
        src={result.dataUrl}
        alt="Rendered component"
        className="object-contain"
      />
      <p className="text-muted-foreground px-2 text-sm">
        Rendered <code>{result.options.format}</code> in{" "}
        <code>{Math.round(result.duration)}ms</code>
      </p>
    </div>
  );
}
