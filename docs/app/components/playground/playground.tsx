"use client";

import { AxeIcon, ExternalLinkIcon, GlobeIcon } from "lucide-react";
import { lazy, Suspense, useEffect, useState } from "react";
import { cn } from "~/lib/utils";
import type { Template } from "~/playground/templates";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "../ui/resizable";
import { ComponentEditor } from "./component-editor";
import { LoadingScreen } from "./loading-screen";
import { LabeledPane, OutputPanel, PDF_VIEWS, type PdfView, type Zoom } from "./output-panel";
import { Toolbar, type TabId } from "./toolbar";
import { DEFAULT_TEMPLATE, useSharedCode } from "./use-shared-code";
import { type RenderSuccess, useRenderWorker } from "./use-render-worker";

const BrowserPreview = lazy(() => import("./browser-preview"));

const RUN_HINT_KEY = "takumi-playground-run-hint";

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

  if (result.notice) parts.push(result.notice);

  return parts.join(" · ");
}

export default function Playground() {
  const { code, setCode, matchedTemplate, isShared } = useSharedCode();
  const [isFormatting, setIsFormatting] = useState(false);
  const [zoom, setZoom] = useState<Zoom>("fit");
  const [pdfView, setPdfView] = useState<PdfView>("preview");
  /** The code the preview was rendered from; `code` runs ahead of it while editing. */
  const [ranCode, setRanCode] = useState<string>();
  const [hintDismissed, setHintDismissed] = useState(true);
  const [copied, setCopied] = useState(false);
  const [activeTab, setActiveTab] = useState<TabId>("code");

  const { isReady, lastSuccess, renderError, browserPreview } = useRenderWorker(ranCode);

  const selectedTemplateName = matchedTemplate?.name ?? "Custom";
  const outputKind = lastSuccess?.outputKind;
  const isStale = code !== undefined && code !== ranCode;
  const isUnrunShare = isShared && ranCode === undefined;

  useEffect(() => {
    setHintDismissed(localStorage.getItem(RUN_HINT_KEY) === "seen");
  }, []);

  // The first render happens on its own; after that the editor waits for Run,
  // so a half-typed line never reloads the preview. Code that arrived in a link
  // is somebody else's, so it waits for Run too.
  useEffect(() => {
    if (ranCode === undefined && code !== undefined && !isShared) setRanCode(code);
  }, [code, ranCode, isShared]);

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
        cssVariables={browserPreview?.cssVariables}
      />
    </Suspense>
  );
  const splitPreview = (
    <ResizablePanelGroup orientation="vertical">
      <ResizablePanel defaultSize={outputKind === "pdf" ? 100 : 50} minSize={20}>
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
      {outputKind !== "pdf" && (
        <>
          <ResizableHandle withHandle className="hover:bg-primary/50 transition-colors" />
          <ResizablePanel defaultSize={50} minSize={20}>
            <LabeledPane label="Browser" icon={GlobeIcon}>
              {browserPane}
            </LabeledPane>
          </ResizablePanel>
        </>
      )}
    </ResizablePanelGroup>
  );

  return (
    <div className="relative flex h-[calc(100dvh-3.5rem)] flex-col bg-background">
      <LoadingScreen done={isReady} />
      <Toolbar
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        selectedTemplateName={selectedTemplateName}
        selectedTemplateId={matchedTemplate?.id}
        onSelectTemplate={loadTemplate}
        isStale={isStale}
        isReady={isReady}
        onRun={run}
        hintDismissed={hintDismissed}
        onDismissHint={dismissHint}
        isFormatting={isFormatting}
        onFormat={formatCode}
        onReset={resetCode}
        outputKind={outputKind}
        zoom={zoom}
        onToggleZoom={() => setZoom(zoom === "fit" ? "actual" : "fit")}
        copied={copied}
        onCopyShareLink={copyShareLink}
        hasOutput={Boolean(lastSuccess)}
        onDownload={downloadOutput}
      />

      {isUnrunShare && (
        <div className="shrink-0 border-b bg-muted/40 px-3 py-1.5 font-mono text-[11px] text-muted-foreground">
          Somebody else wrote this code and sent you the link. Read it before you press Run.
        </div>
      )}

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
          <span className="playground-breathe">loading wasm…</span>
        ) : isUnrunShare ? (
          <span>Waiting for Run.</span>
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
