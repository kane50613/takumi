"use client";

import {
  AxeIcon,
  CheckIcon,
  ChevronDownIcon,
  Code2Icon,
  DownloadIcon,
  GlobeIcon,
  LinkIcon,
  Loader2Icon,
  RotateCcwIcon,
  Wand2Icon,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { lazy, type ReactNode, Suspense, useEffect, useRef, useState } from "react";
import type { z } from "zod/mini";
import { cn } from "~/lib/utils";
import {
  messageSchema,
  type RenderMessageInput,
  type renderResultSchema,
} from "~/playground/schema";
import { compressCode, decompressCode } from "~/playground/share";
import { defaultTemplate, templates } from "~/playground/templates";
import TakumiWorker from "~/playground/worker?worker";
import { Button } from "../ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "../ui/resizable";
import { ComponentEditor } from "./component-editor";

const BrowserPreview = lazy(() => import("./browser-preview"));

const DEFAULT_TEMPLATE = templates[0];

type TabId = "code" | "takumi" | "browser";

const TABS: { id: TabId; label: string; icon: LucideIcon }[] = [
  { id: "code", label: "Code", icon: Code2Icon },
  { id: "takumi", label: "Takumi", icon: AxeIcon },
  { id: "browser", label: "Browser", icon: GlobeIcon },
];

type RenderResult = z.infer<typeof renderResultSchema>["result"];
type RenderSuccess = Extract<RenderResult, { status: "success" }> & { outputSize: number };
type RenderError = Extract<RenderResult, { status: "error" }>;
type Zoom = "fit" | "actual";

function isBlobUrl(url: string | undefined): url is string {
  return typeof url === "string" && url.startsWith("blob:");
}

function formatStats(result: RenderSuccess) {
  const { width = 1200, height = 630 } = result.options;
  const size = `${(result.outputSize / 1024).toFixed(1)} KB`;
  return `${width} × ${height} · ${result.outputFormat.toUpperCase()} · ${size} · ${Math.round(result.duration)} ms`;
}

export default function Playground() {
  const [code, setCode] = useState<string>();
  const [lastSuccess, setLastSuccess] = useState<RenderSuccess>();
  const [browserPreview, setBrowserPreview] = useState<{
    html: string;
    options: RenderSuccess["options"];
  }>();
  const [renderError, setRenderError] = useState<RenderError>();
  const [isReady, setIsReady] = useState(false);
  const [isFormatting, setIsFormatting] = useState(false);
  const [zoom, setZoom] = useState<Zoom>("fit");
  const [copied, setCopied] = useState(false);
  const [searchParams, setSearchParams] = useState(() => {
    if (typeof window === "undefined") {
      return new URLSearchParams();
    }

    return new URLSearchParams(window.location.search);
  });
  const currentRequestIdRef = useRef(0);

  const workerRef = useRef<Worker | undefined>(undefined);
  const [activeTab, setActiveTab] = useState<TabId>("code");

  const codeQuery = searchParams.get("code");
  const templateQuery = searchParams.get("template");
  const matchedTemplate = templates.find((template) => template.code === code);
  const selectedTemplateName = matchedTemplate?.name ?? "Templates";

  useEffect(() => {
    const onPopState = () => {
      setSearchParams(new URLSearchParams(window.location.search));
    };

    window.addEventListener("popstate", onPopState);

    return () => {
      window.removeEventListener("popstate", onPopState);
    };
  }, []);

  const replaceSearchParams = (updater: (current: URLSearchParams) => URLSearchParams) => {
    const next = updater(new URLSearchParams(window.location.search));
    const search = next.toString();
    const url = `${window.location.pathname}${search ? `?${search}` : ""}${window.location.hash}`;

    window.history.replaceState(window.history.state, "", url);
    setSearchParams(next);
  };

  useEffect(() => {
    if (code !== undefined) return;

    let cancelled = false;

    void (async () => {
      const templateCode = templates.find((template) => template.id === templateQuery)?.code;
      const initialCode = codeQuery
        ? await decompressCode(codeQuery)
        : (templateCode ?? DEFAULT_TEMPLATE.code);

      if (!cancelled) {
        setCode(initialCode);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [codeQuery, code, templateQuery]);

  useEffect(() => {
    if (!code) return;

    if (code === defaultTemplate) {
      replaceSearchParams((current) => {
        const next = new URLSearchParams(current);
        next.delete("code");
        next.delete("template");
        return next;
      });
      return;
    }

    if (matchedTemplate) {
      replaceSearchParams((current) => {
        const next = new URLSearchParams(current);
        next.delete("code");
        next.set("template", matchedTemplate.id);
        return next;
      });
      return;
    }

    const timer = setTimeout(() => {
      compressCode(code).then((base64) => {
        replaceSearchParams((current) => {
          const next = new URLSearchParams(current);
          next.delete("template");
          next.set("code", base64);
          return next;
        });
      });
    }, 500);

    return () => clearTimeout(timer);
  }, [code, matchedTemplate]);

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
            setBrowserPreview({ html: message.html, options: message.options });
          }
          break;
        }
        case "render-result": {
          const { result } = message;
          if (result.id !== currentRequestIdRef.current) break;

          if (result.status === "success") {
            const blob = new Blob([result.outputBuffer as BlobPart], {
              type: `image/${result.outputFormat}`,
            });
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
  }, []);

  useEffect(() => {
    if (isReady && code !== undefined) {
      const timer = setTimeout(() => {
        const requestId = currentRequestIdRef.current + 1;
        currentRequestIdRef.current = requestId;
        workerRef.current?.postMessage({
          type: "render-request",
          id: requestId,
          code,
        } satisfies RenderMessageInput);
      }, 300);

      return () => clearTimeout(timer);
    }
  }, [isReady, code]);

  useEffect(() => {
    if (!isBlobUrl(lastSuccess?.outputUrl)) return;

    const url = lastSuccess.outputUrl;
    return () => URL.revokeObjectURL(url);
  }, [lastSuccess]);

  const loadTemplate = (templateCode: string) => {
    setCode(templateCode);
  };
  const resetCode = () => {
    setCode(DEFAULT_TEMPLATE.code);
    setActiveTab("code");
  };

  const formatCode = async () => {
    if (!code) return;
    try {
      setIsFormatting(true);
      const [prettier, prettierPluginEstree, prettierPluginTypeScript] = await Promise.all([
        import("prettier/standalone"),
        import("prettier/plugins/estree"),
        import("prettier/plugins/typescript"),
      ]);

      const formatted = await prettier.format(code, {
        parser: "typescript",
        plugins: [prettierPluginEstree, prettierPluginTypeScript],
      });

      setCode(formatted);
    } catch (error) {
      console.error("Failed to format code:", error);
    } finally {
      setIsFormatting(false);
    }
  };

  const copyShareLink = async () => {
    await navigator.clipboard.writeText(window.location.href);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const downloadImage = () => {
    if (!lastSuccess?.outputUrl) return;

    const { width = 1200, height = 630 } = lastSuccess.options;
    const link = document.createElement("a");
    link.href = lastSuccess.outputUrl;
    link.download = `takumi-${width}x${height}.${lastSuccess.outputFormat}`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  const editor = (
    <div className="relative h-full min-w-0 overflow-hidden">
      {code && <ComponentEditor code={code} setCode={setCode} />}
    </div>
  );
  const takumiPane = (
    <PreviewPanel lastSuccess={lastSuccess} error={renderError} zoom={zoom} isReady={isReady} />
  );
  const browserPane = (
    <Suspense fallback={<div className="h-full bg-muted/20" />}>
      <BrowserPreview
        html={browserPreview?.html}
        width={browserPreview?.options.width}
        height={browserPreview?.options.height}
        stylesheets={browserPreview?.options.stylesheets}
      />
    </Suspense>
  );
  const splitPreview = (
    <ResizablePanelGroup orientation="vertical">
      <ResizablePanel defaultSize={50} minSize={20}>
        <LabeledPane label="Takumi" icon={AxeIcon}>
          {takumiPane}
        </LabeledPane>
      </ResizablePanel>
      <ResizableHandle withHandle className="hover:bg-primary/50 transition-colors" />
      <ResizablePanel defaultSize={50} minSize={20}>
        <LabeledPane label="Browser" icon={GlobeIcon}>
          {browserPane}
        </LabeledPane>
      </ResizablePanel>
    </ResizablePanelGroup>
  );

  return (
    <div className="flex h-[calc(100dvh-3.5rem)] flex-col bg-background">
      <div className="flex h-10 shrink-0 items-center gap-1 border-b px-2 md:px-3">
        <div className="flex items-center rounded-md border p-0.5 md:hidden">
          {TABS.map(({ id, label, icon: Icon }) => (
            <Button
              key={id}
              variant="ghost"
              size="sm"
              className={cn(
                "h-6 gap-1 rounded-sm px-2 font-mono text-xs",
                activeTab === id ? "bg-muted text-foreground" : "text-muted-foreground",
              )}
              onClick={() => setActiveTab(id)}
            >
              <Icon className="size-3" />
              {label}
            </Button>
          ))}
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 min-w-0 max-w-40 px-2 font-mono text-xs text-muted-foreground"
            >
              <span className="truncate">{selectedTemplateName}</span>
              <ChevronDownIcon className="size-3" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            {templates.map((t) => (
              <DropdownMenuItem
                key={t.name}
                onClick={() => loadTemplate(t.code)}
                className="cursor-pointer font-mono text-xs"
              >
                {t.name}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>

        <div className={cn("flex items-center gap-0.5", activeTab !== "code" && "max-md:hidden")}>
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-7 text-muted-foreground"
            onClick={formatCode}
            disabled={isFormatting}
            title="Format code"
          >
            {isFormatting ? (
              <Loader2Icon className="size-3.5 animate-spin" />
            ) : (
              <Wand2Icon className="size-3.5" />
            )}
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-7 text-muted-foreground"
            onClick={resetCode}
            title="Reset code"
          >
            <RotateCcwIcon className="size-3.5" />
          </Button>
        </div>

        <div className="ml-auto flex items-center gap-0.5">
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 font-mono text-xs text-muted-foreground max-md:hidden"
            onClick={() => setZoom(zoom === "fit" ? "actual" : "fit")}
            title="Toggle zoom"
          >
            {zoom === "fit" ? "Fit" : "100%"}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 gap-1.5 px-2 font-mono text-xs text-muted-foreground"
            onClick={copyShareLink}
            title="Copy share link"
          >
            {copied ? (
              <CheckIcon className="size-3.5 text-primary" />
            ) : (
              <LinkIcon className="size-3.5" />
            )}
            <span className="max-md:hidden">{copied ? "Copied" : "Share"}</span>
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-7 text-muted-foreground"
            onClick={downloadImage}
            disabled={!lastSuccess}
            title="Download image"
          >
            <DownloadIcon className="size-3.5" />
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1">
        <div className="hidden h-full md:block">
          <ResizablePanelGroup orientation="horizontal">
            <ResizablePanel defaultSize={55} minSize={30}>
              {editor}
            </ResizablePanel>
            <ResizableHandle className="hover:bg-primary/50 transition-colors" />
            <ResizablePanel defaultSize={45} minSize={30}>
              {splitPreview}
            </ResizablePanel>
          </ResizablePanelGroup>
        </div>

        <div className="h-full md:hidden">
          {activeTab === "code" ? editor : activeTab === "takumi" ? takumiPane : browserPane}
        </div>
      </div>

      <div className="flex h-7 shrink-0 items-center gap-3 border-t px-3 font-mono text-[11px] text-muted-foreground">
        {!isReady ? (
          <span>loading wasm…</span>
        ) : renderError ? (
          <span className="flex min-w-0 items-center gap-2">
            <span className="shrink-0 text-primary">error</span>
            <span className="truncate">{renderError.message.split("\n")[0]}</span>
          </span>
        ) : lastSuccess ? (
          <span className="truncate">{formatStats(lastSuccess)}</span>
        ) : (
          <span>rendering…</span>
        )}
      </div>
    </div>
  );
}

function LabeledPane({
  label,
  icon: Icon,
  children,
}: {
  label: string;
  icon: LucideIcon;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-6 shrink-0 items-center gap-1.5 border-b px-3 font-mono text-[11px] uppercase tracking-wide text-muted-foreground">
        <Icon className="size-3" />
        {label}
      </div>
      <div className="min-h-0 flex-1">{children}</div>
    </div>
  );
}

function PreviewPanel({
  lastSuccess,
  error,
  zoom,
  isReady,
}: {
  lastSuccess: RenderSuccess | undefined;
  error: RenderError | undefined;
  zoom: Zoom;
  isReady: boolean;
}) {
  const image = lastSuccess && (
    <img
      src={lastSuccess.outputUrl}
      alt="Rendered output"
      className={cn(
        "border",
        zoom === "fit" ? "max-h-full max-w-full object-contain" : "max-w-none",
        error && "opacity-40",
      )}
    />
  );

  if (!lastSuccess && !error) {
    return (
      <div className="flex h-full items-center justify-center gap-2 bg-muted/20 font-mono text-xs text-muted-foreground">
        <Loader2Icon className="size-3.5 animate-spin" />
        {isReady ? "rendering…" : "loading wasm…"}
      </div>
    );
  }

  return (
    <div className="relative h-full min-w-0 overflow-hidden bg-muted/20">
      {zoom === "fit" ? (
        <div className="absolute inset-0 flex items-center justify-center">{image}</div>
      ) : (
        <div className="absolute inset-0 overflow-auto">
          <div className="flex h-fit min-h-full w-fit min-w-full items-center justify-center">
            {image}
          </div>
        </div>
      )}
      {error && (
        <div className="absolute inset-x-0 bottom-0 border-t bg-background/95 px-3 py-2 font-mono text-xs">
          <pre className="max-h-40 overflow-auto whitespace-pre-wrap text-muted-foreground">
            {error.message}
          </pre>
        </div>
      )}
    </div>
  );
}
