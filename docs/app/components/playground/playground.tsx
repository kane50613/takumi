import {
  ChevronDownIcon,
  Code2Icon,
  DownloadIcon,
  ImageIcon,
  Loader2Icon,
  RotateCcwIcon,
  Wand2Icon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router";
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

const DEFAULT_TEMPLATE = templates[0];

function isBlobUrl(url: string | undefined): url is string {
  return typeof url === "string" && url.startsWith("blob:");
}

export default function Playground() {
  const [code, setCode] = useState<string>();
  const [rendered, setRendered] = useState<z.infer<typeof renderResultSchema>["result"]>();
  const [isReady, setIsReady] = useState(false);
  const [isFormatting, setIsFormatting] = useState(false);
  const currentRequestIdRef = useRef(0);

  const workerRef = useRef<Worker | undefined>(undefined);
  const [searchParams, setSearchParams] = useSearchParams();
  const [activeTab, setActiveTab] = useState<"code" | "preview">("code");

  const codeQuery = searchParams.get("code");
  const templateQuery = searchParams.get("template");
  const matchedTemplate = templates.find((template) => template.code === code);
  const selectedTemplateName = matchedTemplate?.name ?? "Templates";

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
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          next.delete("code");
          next.delete("template");
          return next;
        },
        { replace: true },
      );
      return;
    }

    if (matchedTemplate) {
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          next.delete("code");
          next.set("template", matchedTemplate.id);
          return next;
        },
        { replace: true },
      );
      return;
    }

    const timer = setTimeout(() => {
      compressCode(code).then((base64) => {
        setSearchParams(
          (prev) => {
            const next = new URLSearchParams(prev);
            next.delete("template");
            next.set("code", base64);
            return next;
          },
          { replace: true },
        );
      });
    }, 500);

    return () => clearTimeout(timer);
  }, [code, matchedTemplate, setSearchParams]);

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
          if (message.result.status === "success") {
            message.result.outputUrl = URL.createObjectURL(
              new Blob([message.result.outputBuffer as BlobPart], {
                type: `image/${message.result.outputFormat}`,
              }),
            );
          }

          if (message.result.id === currentRequestIdRef.current) {
            setRendered(message.result);
          } else if (message.result.status === "success" && message.result.outputUrl) {
            URL.revokeObjectURL(message.result.outputUrl);
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
    if (rendered?.status !== "success" || !isBlobUrl(rendered.outputUrl)) {
      return;
    }

    return () => {
      if (rendered.outputUrl) {
        URL.revokeObjectURL(rendered.outputUrl);
      }
    };
  }, [rendered]);

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

  const mobileHeader = (
    <div className="flex md:hidden shrink-0 items-center gap-2 border-b border-zinc-800/80 bg-background px-3 py-2">
      <div className="flex min-w-0 bg-zinc-900/40 rounded-lg p-0.5 gap-0.5 border border-zinc-800/50">
        <Button
          variant={activeTab === "code" ? "secondary" : "ghost"}
          size="sm"
          className={cn(
            "h-7 rounded-md text-[11px] font-bold transition-all",
            activeTab === "code" ? "px-2.5" : "px-2",
            activeTab === "code" && "bg-zinc-800 text-white shadow-sm ring-1 ring-zinc-700/50",
          )}
          onClick={() => setActiveTab("code")}
        >
          <Code2Icon className={cn("h-3 w-3", activeTab === "code" && "mr-1.5")} />
          {activeTab === "code" && "Code"}
        </Button>
        <Button
          variant={activeTab === "preview" ? "secondary" : "ghost"}
          size="sm"
          className={cn(
            "h-7 rounded-md text-[11px] font-bold transition-all",
            activeTab === "preview" ? "px-2.5" : "px-2",
            activeTab === "preview" && "bg-zinc-800 text-white shadow-sm ring-1 ring-zinc-700/50",
          )}
          onClick={() => setActiveTab("preview")}
        >
          <ImageIcon className={cn("h-3 w-3", activeTab === "preview" && "mr-1.5")} />
          {activeTab === "preview" && "Preview"}
        </Button>
      </div>

      <div className="ml-auto flex min-w-0 items-center gap-1">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 min-w-0 max-w-40 px-2 text-[11px] font-bold text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50"
            >
              <span className="truncate">{selectedTemplateName}</span>
              <ChevronDownIcon className="ml-1 h-3 w-3 text-zinc-500" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="end"
            className="w-45 bg-zinc-950 border-zinc-800 text-zinc-300"
          >
            {templates.map((t) => (
              <DropdownMenuItem
                key={t.name}
                onClick={() => loadTemplate(t.code)}
                className="text-xs focus:bg-zinc-900 focus:text-zinc-100 cursor-pointer"
              >
                {t.name}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>

        {activeTab === "code" && (
          <div className="flex shrink-0 items-center border-l border-zinc-800 ml-0.5 pl-1 gap-0.5">
            <Button
              variant="ghost"
              size="icon-sm"
              className="h-7 w-7 text-zinc-500 hover:text-zinc-200"
              onClick={formatCode}
              disabled={isFormatting}
              title="Format Code"
            >
              {isFormatting ? (
                <Loader2Icon className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Wand2Icon className="h-3.5 w-3.5" />
              )}
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              className="h-7 w-7 text-zinc-500 hover:text-red-400"
              onClick={resetCode}
              title="Reset Code"
            >
              <RotateCcwIcon className="h-3.5 w-3.5" />
            </Button>
          </div>
        )}
      </div>
    </div>
  );

  return (
    <div className="flex h-[calc(100dvh-3.5rem)] flex-col bg-[#09090b]">
      {mobileHeader}

      <div className="flex-1 min-h-0">
        <div className="hidden md:block h-full">
          <ResizablePanelGroup orientation="horizontal">
            <ResizablePanel defaultSize={55} minSize={30}>
              <CodePanel
                code={code}
                setCode={setCode}
                formatCode={formatCode}
                isFormatting={isFormatting}
                loadTemplate={loadTemplate}
                resetCode={resetCode}
                selectedTemplateName={selectedTemplateName}
              />
            </ResizablePanel>
            <ResizableHandle
              withHandle
              className="bg-zinc-800/50 hover:bg-zinc-700 transition-colors"
            />
            <ResizablePanel defaultSize={45} minSize={30}>
              <PreviewPanel rendered={rendered} />
            </ResizablePanel>
          </ResizablePanelGroup>
        </div>

        <div className="md:hidden h-full">
          {activeTab === "code" ? (
            <CodePanel
              code={code}
              setCode={setCode}
              formatCode={formatCode}
              isFormatting={isFormatting}
              loadTemplate={loadTemplate}
              resetCode={resetCode}
              selectedTemplateName={selectedTemplateName}
            />
          ) : (
            <PreviewPanel rendered={rendered} />
          )}
        </div>
      </div>
    </div>
  );
}

function CodePanel({
  code,
  setCode,
  formatCode,
  isFormatting,
  loadTemplate,
  resetCode,
  selectedTemplateName,
}: {
  code: string | undefined;
  setCode: React.Dispatch<React.SetStateAction<string | undefined>>;
  formatCode: () => void;
  isFormatting: boolean;
  loadTemplate: (templateCode: string) => void;
  resetCode: () => void;
  selectedTemplateName: string;
}) {
  return (
    <div className="flex h-full min-w-0 flex-col overflow-hidden bg-zinc-950/50">
      <div className="hidden md:flex h-10 shrink-0 items-center justify-between border-b border-zinc-800 bg-zinc-950/40 px-4">
        <p className="flex items-center text-xs font-bold uppercase tracking-widest text-zinc-500">
          <Code2Icon className="mr-1.5 h-4 w-4" />
          Editor
        </p>
        <div className="flex gap-1.5 items-center">
          <div className="hidden sm:flex items-center gap-2 mr-2 border-r border-zinc-800 pr-4">
            <span className="text-[10px] uppercase font-bold text-zinc-600 tracking-wider">
              Templates:
            </span>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2.5 text-[11px] font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900/50 border border-zinc-800 bg-zinc-950/50 min-w-30 justify-between"
                >
                  {selectedTemplateName}
                  <ChevronDownIcon className="ml-1 h-3 w-3 text-zinc-500" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent
                align="start"
                className="w-45 bg-zinc-950 border-zinc-800 text-zinc-300"
              >
                {templates.map((t) => (
                  <DropdownMenuItem
                    key={t.name}
                    onClick={() => loadTemplate(t.code)}
                    className="text-xs focus:bg-zinc-900 focus:text-zinc-100 cursor-pointer"
                  >
                    {t.name}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
          <Button
            variant="ghost"
            size="icon-sm"
            className="h-7 w-7 text-zinc-400 hover:text-zinc-200"
            onClick={formatCode}
            disabled={isFormatting}
            title="Format"
          >
            {isFormatting ? (
              <Loader2Icon className="h-4 w-4 animate-spin" />
            ) : (
              <Wand2Icon className="h-4 w-4" />
            )}
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            className="h-7 w-7 text-zinc-400 hover:text-red-400"
            onClick={resetCode}
            title="Reset"
          >
            <RotateCcwIcon className="h-4 w-4" />
          </Button>
        </div>
      </div>
      <div className="relative min-h-0 min-w-0 flex-1 overflow-hidden">
        {code && <ComponentEditor code={code} setCode={setCode} />}
      </div>
    </div>
  );
}

function PreviewPanel({
  rendered,
}: {
  rendered: z.infer<typeof renderResultSchema>["result"] | undefined;
}) {
  return (
    <div className="flex h-full min-w-0 flex-col overflow-hidden bg-[#09090b]">
      <div className="hidden md:flex h-10 shrink-0 items-center justify-between border-b border-zinc-800 bg-zinc-950/40 px-4">
        <p className="flex items-center text-xs font-bold uppercase tracking-widest text-zinc-500">
          <ImageIcon className="mr-1.5 h-4 w-4" />
          Preview
        </p>
      </div>
      <div className="flex-1 w-full overflow-y-auto">
        {rendered && <RenderPreview result={rendered} />}
      </div>
    </div>
  );
}

function RenderPreview({ result }: { result: z.infer<typeof renderResultSchema>["result"] }) {
  if (result.status === "error") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center p-8 bg-red-950/10">
        <div className="bg-red-950/30 border border-red-900/50 p-6 rounded-xl flex flex-col items-center max-w-2xl w-full">
          <p className="mb-4 text-xl font-bold text-red-400">Error Rendering</p>
          <pre className="w-full whitespace-pre-wrap rounded-lg bg-black/60 p-5 text-xs text-red-300 shadow-inner overflow-x-auto leading-relaxed border border-red-900/20">
            {result.message}
          </pre>
        </div>
      </div>
    );
  }

  return (
    <div className="relative flex h-full w-full flex-col items-center justify-center gap-8 p-4 sm:p-8">
      <div
        className="relative shadow-2xl overflow-hidden rounded-xl border border-zinc-800/60 transition-all hover:shadow-emerald-500/5 hover:border-zinc-700/80"
        style={{
          backgroundImage:
            "radial-gradient(circle at 10px 10px, #18181b 2%, transparent 0%), radial-gradient(circle at 25px 25px, #18181b 2%, transparent 0%)",
          backgroundSize: "30px 30px",
          backgroundColor: "#09090b",
        }}
      >
        <img
          src={result.outputUrl}
          alt="Rendered component"
          className="object-contain"
          style={{
            maxWidth: "100%",
            maxHeight: "calc(100vh - 14rem)",
          }}
        />
      </div>
      <div className="flex items-center gap-3 text-xs font-medium">
        <div className="flex h-9 items-center rounded-full border border-zinc-800/80 bg-zinc-900/80 pl-4 pr-2 text-zinc-400 shadow-lg backdrop-blur-md">
          <span>Format</span>
          <span className="ml-2 rounded bg-zinc-950 px-1.5 py-0.5 text-zinc-100 font-mono">
            {result.outputFormat.toUpperCase()}
          </span>
          <div className="mx-3 h-4 w-px bg-zinc-800" />
          <div className="flex items-center">
            <div className="mr-2 h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.5)]" />
            <span>Time</span>
          </div>
          <span className="ml-2 rounded bg-zinc-950 px-1.5 py-0.5 text-emerald-400 font-mono">
            {Math.round(result.duration)}ms
          </span>
        </div>
        <Button
          variant="default"
          size="sm"
          className="h-9 rounded-full bg-zinc-100 px-4! font-semibold text-zinc-900 shadow-lg transition-transform hover:scale-105 hover:bg-white active:scale-95"
          onClick={() => {
            if (result.outputUrl) {
              const link = document.createElement("a");
              link.href = result.outputUrl;
              link.download = `takumi-image.${result.outputFormat}`;
              document.body.appendChild(link);
              link.click();
              document.body.removeChild(link);
            }
          }}
        >
          <DownloadIcon className="h-3.5 w-3.5" />
          Download
        </Button>
      </div>
    </div>
  );
}
