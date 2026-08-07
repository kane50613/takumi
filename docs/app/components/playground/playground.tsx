"use client";

import {
  AxeIcon,
  CheckIcon,
  ChevronDownIcon,
  Code2Icon,
  DownloadIcon,
  ExternalLinkIcon,
  EyeIcon,
  FileTextIcon,
  FilmIcon,
  GlobeIcon,
  ImageIcon,
  LinkIcon,
  Loader2Icon,
  PlayIcon,
  RotateCcwIcon,
  Wand2Icon,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { lazy, type ReactNode, Suspense, useEffect, useRef, useState } from "react";
import type { z } from "zod/mini";
import { cn } from "~/lib/utils";
import {
  messageSchema,
  type OutputKind,
  outputKinds,
  type RenderMessageInput,
  type renderResultSchema,
} from "~/playground/schema";
import type { PdfInspection } from "~/playground/inspect-pdf";
import { compressCode, decompressCode } from "~/playground/share";
import { defaultTemplate, type Template, templates } from "~/playground/templates";
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

const RUN_HINT_KEY = "takumi-playground-run-hint";

type TabId = "code" | "preview";

const TABS: { id: TabId; label: string; icon: LucideIcon }[] = [
  { id: "code", label: "Code", icon: Code2Icon },
  { id: "preview", label: "Preview", icon: EyeIcon },
];

const KINDS: Record<OutputKind, { label: string; icon: LucideIcon }> = {
  image: { label: "Images", icon: ImageIcon },
  animation: { label: "Animations", icon: FilmIcon },
  pdf: { label: "Documents", icon: FileTextIcon },
};

type PdfView = "preview" | "document";

const PDF_VIEWS: { id: PdfView; label: string }[] = [
  { id: "preview", label: "Preview" },
  { id: "document", label: "Document" },
];

type RenderResult = z.infer<typeof renderResultSchema>["result"];
type RenderSuccess = Extract<RenderResult, { status: "success" }> & { outputSize: number };
type RenderError = Extract<RenderResult, { status: "error" }>;
type Zoom = "fit" | "actual";

function isBlobUrl(url: string | undefined): url is string {
  return typeof url === "string" && url.startsWith("blob:");
}

function mimeType(result: RenderResult & { status: "success" }) {
  return result.outputKind === "pdf" ? "application/pdf" : `image/${result.outputFormat}`;
}

function fileName(result: RenderSuccess) {
  const slug = result.label.replace(/[^a-z0-9]+/gi, "-").toLowerCase();
  return `takumi-${slug}.${result.outputFormat}`;
}

function formatStats(result: RenderSuccess) {
  const parts = [result.label];
  const pages = result.inspection?.pages;

  if (pages) parts.push(`${pages} page${pages === 1 ? "" : "s"}`);

  parts.push(
    result.outputFormat.toUpperCase(),
    `${(result.outputSize / 1024).toFixed(1)} KB`,
    `${Math.round(result.duration)} ms`,
  );

  return parts.join(" · ");
}

export default function Playground() {
  const [code, setCode] = useState<string>();
  const [lastSuccess, setLastSuccess] = useState<RenderSuccess>();
  const [browserPreview, setBrowserPreview] = useState<{
    html: string;
    width?: number;
    height?: number;
    padding?: string;
    cssContents?: string[];
  }>();
  const [renderError, setRenderError] = useState<RenderError>();
  const [isReady, setIsReady] = useState(false);
  const [isFormatting, setIsFormatting] = useState(false);
  const [zoom, setZoom] = useState<Zoom>("fit");
  const [pdfView, setPdfView] = useState<PdfView>("preview");
  /** The code the preview was rendered from; `code` runs ahead of it while editing. */
  const [ranCode, setRanCode] = useState<string>();
  const [hintDismissed, setHintDismissed] = useState(true);
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
  const selectedTemplateName = matchedTemplate?.name ?? "Custom";
  const outputKind = lastSuccess?.outputKind;
  const isStale = code !== undefined && ranCode !== undefined && code !== ranCode;

  useEffect(() => {
    setHintDismissed(localStorage.getItem(RUN_HINT_KEY) === "seen");
  }, []);

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
  }, []);

  useEffect(() => {
    if (!isReady || ranCode === undefined) return;

    const requestId = currentRequestIdRef.current + 1;
    currentRequestIdRef.current = requestId;
    workerRef.current?.postMessage({
      type: "render-request",
      id: requestId,
      code: ranCode,
    } satisfies RenderMessageInput);
  }, [isReady, ranCode]);

  // The first render happens on its own; after that the editor waits for Run,
  // so a half-typed line never reloads the preview.
  useEffect(() => {
    if (ranCode === undefined && code !== undefined) setRanCode(code);
  }, [code, ranCode]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        run();
      }
    };

    window.addEventListener("keydown", onKeyDown);

    return () => window.removeEventListener("keydown", onKeyDown);
  });

  useEffect(() => {
    if (!isBlobUrl(lastSuccess?.outputUrl)) return;

    const url = lastSuccess.outputUrl;
    return () => URL.revokeObjectURL(url);
  }, [lastSuccess]);

  const run = () => {
    setRanCode(code);
    dismissHint();
  };
  const dismissHint = () => {
    setHintDismissed(true);
    localStorage.setItem(RUN_HINT_KEY, "seen");
  };
  // Picking a template or resetting is its own deliberate action, so those
  // render straight away instead of waiting for Run.
  const loadTemplate = (template: Template) => {
    setCode(template.code);
    setRanCode(template.code);
  };
  const resetCode = () => {
    setCode(DEFAULT_TEMPLATE.code);
    setRanCode(DEFAULT_TEMPLATE.code);
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
      // Formatting cannot change the output, so it must not make the preview stale.
      if (!isStale) setRanCode(formatted);
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

  const downloadOutput = () => {
    if (!lastSuccess?.outputUrl) return;

    const link = document.createElement("a");
    link.href = lastSuccess.outputUrl;
    link.download = fileName(lastSuccess);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  const editor = (
    <div className="relative h-full min-w-0 overflow-hidden">
      {code && <ComponentEditor code={code} setCode={setCode} onRun={run} />}
    </div>
  );
  const takumiPane = (
    <OutputPanel
      lastSuccess={lastSuccess}
      error={renderError}
      zoom={zoom}
      isReady={isReady}
      pdfView={pdfView}
    />
  );
  const browserPane = (
    <Suspense fallback={<div className="h-full bg-muted/20" />}>
      <BrowserPreview
        html={browserPreview?.html}
        width={browserPreview?.width}
        height={browserPreview?.height}
        padding={browserPreview?.padding}
        cssContents={browserPreview?.cssContents}
      />
    </Suspense>
  );
  const splitPreview = (
    <ResizablePanelGroup orientation="vertical">
      <ResizablePanel defaultSize={50} minSize={20}>
        <LabeledPane
          label={outputKind === "pdf" ? "Takumi PDF" : "Takumi"}
          icon={AxeIcon}
          actions={
            outputKind === "pdf" && (
              <>
                {PDF_VIEWS.map(({ id, label }) => (
                  <button
                    key={id}
                    type="button"
                    onClick={() => setPdfView(id)}
                    className={cn(
                      "rounded-sm px-1.5 py-0.5 uppercase transition-colors hover:text-foreground",
                      pdfView === id && "bg-muted text-foreground",
                    )}
                  >
                    {label}
                  </button>
                ))}
                {/* Mobile browsers mostly refuse to paint a PDF inside a frame,
                    so the file needs a way out to the viewer. */}
                {lastSuccess?.outputUrl && (
                  <a
                    href={lastSuccess.outputUrl}
                    target="_blank"
                    rel="noreferrer"
                    title="Open the PDF in a new tab"
                    className="rounded-sm px-1 py-0.5 transition-colors hover:text-foreground"
                  >
                    <ExternalLinkIcon className="size-3" />
                  </a>
                )}
              </>
            )
          }
        >
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
      <div className="flex h-10 shrink-0 items-center gap-1 overflow-x-auto border-b px-2 md:px-3">
        <div className="flex shrink-0 items-center rounded-md border p-0.5 md:hidden">
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
              <span className="max-[400px]:hidden">{label}</span>
            </Button>
          ))}
        </div>

        <TemplateMenu
          selectedName={selectedTemplateName}
          selectedId={matchedTemplate?.id}
          onSelect={loadTemplate}
        />

        <div className="relative flex shrink-0 items-center gap-0.5">
          <Button
            variant={isStale ? "default" : "ghost"}
            size="sm"
            className="h-7 gap-1.5 px-2 font-mono text-xs"
            onClick={run}
            disabled={!isStale || !isReady}
            title="Run the code (⌘↵)"
          >
            <PlayIcon className="size-3.5" />
            <span className="max-md:hidden">Run</span>
          </Button>
          {isStale && !hintDismissed && (
            <div className="absolute top-9 left-0 z-20 w-56 rounded-md border bg-popover p-3 text-xs shadow-md">
              <p className="m-0 text-popover-foreground">
                The preview waits for you. Hit Run, or press ⌘↵, to render what you just wrote.
              </p>
              <button
                type="button"
                onClick={dismissHint}
                className="mt-2 font-mono text-[11px] text-muted-foreground underline"
              >
                Got it
              </button>
            </div>
          )}
        </div>

        <div
          className={cn(
            "flex shrink-0 items-center gap-0.5",
            activeTab !== "code" && "max-md:hidden",
          )}
        >
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

        <div className="ml-auto flex shrink-0 items-center gap-0.5">
          {outputKind !== "pdf" && (
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2 font-mono text-xs text-muted-foreground max-md:hidden"
              onClick={() => setZoom(zoom === "fit" ? "actual" : "fit")}
              title="Toggle zoom"
            >
              {zoom === "fit" ? "Fit" : "100%"}
            </Button>
          )}
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
            onClick={downloadOutput}
            disabled={!lastSuccess}
            title="Download output"
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

        <div className="h-full md:hidden">{activeTab === "code" ? editor : splitPreview}</div>
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

function TemplateMenu({
  selectedName,
  selectedId,
  onSelect,
}: {
  selectedName: string;
  selectedId: string | undefined;
  onSelect: (template: Template) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 min-w-0 max-w-28 px-2 font-mono text-xs text-muted-foreground md:max-w-40"
        >
          <span className="truncate">{selectedName}</span>
          <ChevronDownIcon className="size-3" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="max-w-72">
        {outputKinds.map((kind) => {
          const group = templates.filter((template) => template.kind === kind);
          if (group.length === 0) return null;

          const { label, icon: Icon } = KINDS[kind];

          return (
            <div key={kind} className="border-b py-1 last:border-b-0">
              <div className="flex items-center gap-1.5 px-2 py-1 font-mono text-[10px] uppercase tracking-wide text-muted-foreground">
                <Icon className="size-3" />
                {label}
              </div>
              {group.map((template) => (
                <DropdownMenuItem
                  key={template.id}
                  onClick={() => onSelect(template)}
                  className="cursor-pointer flex-col items-start gap-0.5"
                >
                  <span className="flex items-center gap-1.5 font-mono text-xs">
                    {template.name}
                    {template.id === selectedId && <CheckIcon className="size-3 text-primary" />}
                  </span>
                  <span className="text-[11px] text-muted-foreground">{template.description}</span>
                </DropdownMenuItem>
              ))}
            </div>
          );
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function LabeledPane({
  label,
  icon: Icon,
  actions,
  children,
}: {
  label: string;
  icon: LucideIcon;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-6 shrink-0 items-center gap-1.5 border-b px-3 font-mono text-[11px] uppercase tracking-wide text-muted-foreground">
        <Icon className="size-3" />
        {label}
        {actions && <div className="ml-auto flex items-center gap-1">{actions}</div>}
      </div>
      <div className="min-h-0 flex-1">{children}</div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex gap-3 border-b py-1.5 last:border-b-0">
      <span className="w-24 shrink-0 text-muted-foreground">{label}</span>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}

/**
 * Reads the rendered bytes back, so the options in the editor are not the only
 * evidence that the document carries what it claims.
 */
function DocumentPanel({ inspection }: { inspection: PdfInspection }) {
  return (
    <div className="h-full overflow-auto bg-muted/20 px-4 py-3 font-mono text-xs">
      <Field label="Standards">
        {inspection.standards.length > 0 ? (
          <span className="text-primary">{inspection.standards.join(" · ")}</span>
        ) : (
          <span className="text-muted-foreground">plain PDF</span>
        )}
      </Field>
      <Field label="Tagged">{inspection.tagged ? "yes" : "no"}</Field>
      <Field label="Pages">{inspection.pages}</Field>
      {inspection.title && <Field label="Title">{inspection.title}</Field>}
      {inspection.authors && <Field label="Authors">{inspection.authors.join(", ")}</Field>}
      {inspection.created && <Field label="Created">{inspection.created}</Field>}
      <Field label="Bookmarks">
        {inspection.bookmarks.length === 0 ? (
          <span className="text-muted-foreground">none</span>
        ) : (
          inspection.bookmarks.map((bookmark, index) => (
            <div
              key={`${bookmark.title}-${index}`}
              style={{ paddingLeft: bookmark.depth * 12 }}
              className="truncate"
            >
              {bookmark.title}
            </div>
          ))
        )}
      </Field>
      <Field label="Attachments">
        {inspection.attachments.length === 0 ? (
          <span className="text-muted-foreground">none</span>
        ) : (
          inspection.attachments.map((attachment) => (
            <div key={attachment.name} className="truncate">
              {attachment.name}
              {attachment.description && (
                <span className="text-muted-foreground"> — {attachment.description}</span>
              )}
            </div>
          ))
        )}
      </Field>
      <p className="mt-3 text-[11px] leading-5 text-muted-foreground">
        These come from the rendered bytes: the page tree, the outline, the file specs and the XMP
        packet. A standard listed here is what the file claims about itself. Takumi checks the claim
        with veraPDF in CI.
      </p>
    </div>
  );
}

function PdfPreview({ url, dimmed }: { url: string | undefined; dimmed: boolean }) {
  if (!url) return null;

  return (
    <object
      data={url}
      type="application/pdf"
      aria-label="Rendered PDF"
      className={cn("size-full", dimmed && "opacity-40")}
    >
      {/* Most mobile browsers have no inline PDF viewer, and `object` renders
          this instead of an empty frame. */}
      <div className="flex h-full items-center justify-center p-6 text-center font-mono text-xs text-muted-foreground">
        <a href={url} target="_blank" rel="noreferrer" className="underline">
          This browser cannot show the PDF here. Open it in a new tab.
        </a>
      </div>
    </object>
  );
}

function OutputPanel({
  lastSuccess,
  error,
  zoom,
  isReady,
  pdfView,
}: {
  lastSuccess: RenderSuccess | undefined;
  error: RenderError | undefined;
  zoom: Zoom;
  isReady: boolean;
  pdfView: PdfView;
}) {
  if (!lastSuccess && !error) {
    return (
      <div className="flex h-full items-center justify-center gap-2 bg-muted/20 font-mono text-xs text-muted-foreground">
        <Loader2Icon className="size-3.5 animate-spin" />
        {isReady ? "rendering…" : "loading wasm…"}
      </div>
    );
  }

  // The browser's own PDF viewer brings paging, zoom and text selection, which
  // is the point of the format.
  if (lastSuccess?.outputKind === "pdf" && pdfView === "document" && lastSuccess.inspection) {
    return <DocumentPanel inspection={lastSuccess.inspection} />;
  }

  const output =
    lastSuccess &&
    (lastSuccess.outputKind === "pdf" ? (
      <PdfPreview url={lastSuccess.outputUrl} dimmed={Boolean(error)} />
    ) : (
      <img
        src={lastSuccess.outputUrl}
        alt="Rendered output"
        className={cn(
          "border",
          zoom === "fit" ? "max-h-full max-w-full object-contain" : "max-w-none",
          error && "opacity-40",
        )}
      />
    ));

  return (
    <div className="relative h-full min-w-0 overflow-hidden bg-muted/20">
      {lastSuccess?.outputKind === "pdf" ? (
        <div className="absolute inset-0">{output}</div>
      ) : zoom === "fit" ? (
        <div className="absolute inset-0 flex items-center justify-center">{output}</div>
      ) : (
        <div className="absolute inset-0 overflow-auto">
          <div className="flex h-fit min-h-full w-fit min-w-full items-center justify-center">
            {output}
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
