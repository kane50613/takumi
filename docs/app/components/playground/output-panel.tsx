import { Loader2Icon } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { type ReactNode, useLayoutEffect, useRef, useState } from "react";
import { cn } from "~/lib/utils";
import type { Rect } from "takumi-paint";
import type { PdfInspection, PdfObject } from "~/playground/inspect-pdf";
import type { RenderError, RenderSuccess } from "./use-render-worker";

export type PdfView = "preview" | "document" | "objects";

export const PDF_VIEWS: { id: PdfView; label: string }[] = [
  { id: "preview", label: "Preview" },
  { id: "document", label: "Document" },
  { id: "objects", label: "Objects" },
];

export type Zoom = "fit" | "actual";

/** A region of the render to zoom onto, in the render's own pixels. */
export type FocusTarget = { rect: Rect; width: number; height: number };

/** The largest enlargement a focus zoom applies. */
const MAX_FOCUS_SCALE = 16;

/** Room left around a focused region, as a share of the pane. */
const FOCUS_FILL = 0.7;

/** Switches between a pane's views. */
export function ViewToggle<View extends string>({
  views,
  value,
  onChange,
}: {
  views: { id: View; label: string }[];
  value: View;
  onChange: (view: View) => void;
}) {
  return views.map(({ id, label }) => (
    <button
      key={id}
      type="button"
      onClick={() => onChange(id)}
      className={cn(
        "rounded-sm px-1.5 py-0.5 uppercase transition-colors hover:text-foreground",
        value === id && "bg-muted text-foreground",
      )}
    >
      {label}
    </button>
  ));
}

export function LabeledPane({
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
      <Field label="Text">
        {inspection.pageText.map((page, index) => (
          <div key={`page-${index + 1}`}>
            page {index + 1}: {page.blocks} blocks / {page.words} words
          </div>
        ))}
      </Field>
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
    </div>
  );
}

const objectId = (number: string) => `pdf-object-${number}`;

/** Turns every `12 0 R` in a dictionary into a jump to that object. */
function linkReferences(dict: string, onJump: (number: string) => void): ReactNode[] {
  return dict.split(/(\d+ 0 R)/g).map((part, index) => {
    const number = part.match(/^(\d+) 0 R$/)?.[1];

    if (!number) return part;

    return (
      <button
        key={`${index}-${part}`}
        type="button"
        onClick={() => onJump(number)}
        className="text-primary underline underline-offset-2"
      >
        {part}
      </button>
    );
  });
}

/** The file as written: every object's dictionary, and the streams that read as text. */
function ObjectsPanel({ objects }: { objects: PdfObject[] }) {
  const [query, setQuery] = useState("");
  const [opened, setOpened] = useState<string[]>([]);

  const needle = query.toLowerCase();
  const matches = objects.filter((object) =>
    `${object.number} ${object.label} ${object.dict} ${object.text ?? ""} ${object.body ?? ""}`
      .toLowerCase()
      .includes(needle),
  );

  const toggle = (number: string, open: boolean) =>
    setOpened((current) =>
      open ? [...new Set([...current, number])] : current.filter((entry) => entry !== number),
    );

  // The target may be filtered out, so the scroll waits for the cleared list to paint.
  const jump = (number: string) => {
    setQuery("");
    toggle(number, true);
    requestAnimationFrame(() => document.getElementById(objectId(number))?.scrollIntoView());
  };

  return (
    <div className="flex h-full flex-col bg-muted/20 font-mono text-xs">
      <div className="flex shrink-0 items-center gap-3 border-b px-4 py-2">
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Filter by number, type, dictionary, text or operator"
          aria-label="Filter objects"
          className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
        />
        <span className="shrink-0 text-muted-foreground">
          {matches.length === objects.length
            ? `${objects.length} objects`
            : `${matches.length} of ${objects.length}`}
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-4 py-1">
        {matches.map((object) => (
          <details
            key={object.number}
            id={objectId(object.number)}
            open={opened.includes(object.number)}
            onToggle={(event) => toggle(object.number, event.currentTarget.open)}
            className="scroll-mt-1 border-b py-1.5 last:border-b-0"
          >
            <summary className="cursor-pointer select-none">
              {object.number} 0 obj
              {object.label && <span className="text-muted-foreground"> · {object.label}</span>}
            </summary>
            <pre className="mt-1 whitespace-pre-wrap break-all text-muted-foreground">
              {linkReferences(object.dict, jump)}
            </pre>
            {object.text !== undefined && (
              <p className="mt-1 whitespace-pre-wrap break-words border-l-2 pl-2">
                {object.text || <span className="text-muted-foreground">draws no text</span>}
              </p>
            )}
            {object.body && <pre className="mt-1 whitespace-pre-wrap break-all">{object.body}</pre>}
          </details>
        ))}
      </div>
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

/** The pane before any output exists: a review prompt for shared code, a progress line otherwise. */
function IdlePane({ isReady, waitingForRun }: { isReady: boolean; waitingForRun?: boolean }) {
  if (waitingForRun) {
    return (
      <div className="flex h-full items-center justify-center bg-muted/20 font-mono text-xs text-muted-foreground">
        Read the code, then press Run.
      </div>
    );
  }

  return (
    <div className="flex h-full items-center justify-center gap-2 bg-muted/20 font-mono text-xs text-muted-foreground">
      <Loader2Icon className="size-3.5 animate-spin" />
      <span className={isReady ? undefined : "playground-breathe"}>
        {isReady ? "rendering…" : "loading wasm…"}
      </span>
    </div>
  );
}

const IDENTITY = { scale: 1, x: 0, y: 0 };

/** The rendered image, with the inspector's frames over it and its focus zoom applied. */
function ImageOutput({
  url,
  zoom,
  dimmed,
  overlay,
  focus,
}: {
  url: string | undefined;
  zoom: Zoom;
  dimmed: boolean;
  overlay?: (zoomScale: number) => ReactNode;
  focus?: FocusTarget;
}) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const frameRef = useRef<HTMLDivElement>(null);
  const [focusZoom, setFocusZoom] = useState(IDENTITY);

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    const frame = frameRef.current;

    if (!focus || !viewport || !frame) {
      setFocusZoom(IDENTITY);
      return;
    }

    const measure = () => {
      if (frame.offsetWidth === 0) return;

      const { rect } = focus;
      const unit = frame.offsetWidth / focus.width;
      const centerX = (rect.x + rect.width / 2) * unit;
      const centerY = (rect.y + rect.height / 2) * unit;

      if (zoom === "actual") {
        setFocusZoom(IDENTITY);
        viewport.scrollTo({
          left: frame.offsetLeft + centerX - viewport.clientWidth / 2,
          top: frame.offsetTop + centerY - viewport.clientHeight / 2,
          behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
        });
        return;
      }

      const fill = Math.min(
        viewport.clientWidth / (rect.width * unit),
        viewport.clientHeight / (rect.height * unit),
      );
      const scale = Math.min(Math.max(fill * FOCUS_FILL, 1), MAX_FOCUS_SCALE);

      setFocusZoom({
        scale,
        x: -(centerX - frame.offsetWidth / 2) * scale,
        y: -(centerY - frame.offsetHeight / 2) * scale,
      });
    };

    measure();

    const observer = new ResizeObserver(measure);

    observer.observe(viewport);
    observer.observe(frame);

    return () => observer.disconnect();
  }, [focus, zoom]);

  const image = (
    <div
      ref={frameRef}
      className="relative shrink-0 border transition-transform duration-250 ease-[cubic-bezier(0.77,0,0.175,1)] motion-reduce:transition-none"
      style={{
        transform: `translate(${focusZoom.x}px, ${focusZoom.y}px) scale(${focusZoom.scale})`,
      }}
    >
      <img
        src={url}
        alt="Rendered output"
        className={cn(
          "block",
          zoom === "fit" ? "max-h-[calc(100cqh-2px)] max-w-[calc(100cqw-2px)]" : "max-w-none",
          dimmed && "opacity-40",
        )}
        // Zoomed in, the render's own pixels are what is being inspected.
        style={{ imageRendering: focusZoom.scale >= 2 ? "pixelated" : undefined }}
      />
      {overlay?.(focusZoom.scale)}
    </div>
  );

  return zoom === "fit" ? (
    <div
      ref={viewportRef}
      className="absolute inset-0 flex items-center justify-center"
      style={{ containerType: "size" }}
    >
      {image}
    </div>
  ) : (
    <div ref={viewportRef} className="absolute inset-0 overflow-auto">
      <div className="flex h-fit min-h-full w-fit min-w-full items-center justify-center">
        {image}
      </div>
    </div>
  );
}

export function OutputPanel({
  lastSuccess,
  error,
  zoom,
  isReady,
  pdfView,
  waitingForRun,
  overlay,
  focus,
}: {
  lastSuccess: RenderSuccess | undefined;
  error: RenderError | undefined;
  zoom: Zoom;
  isReady: boolean;
  pdfView: PdfView;
  /** Shared code renders nothing until the reader has pressed Run themselves. */
  waitingForRun?: boolean;
  /** Drawn over an image render, in its box. */
  overlay?: (zoomScale: number) => ReactNode;
  focus?: FocusTarget;
}) {
  if (!lastSuccess && !error) {
    return <IdlePane isReady={isReady} waitingForRun={waitingForRun} />;
  }

  // The browser's own PDF viewer brings paging, zoom and text selection, which
  // is the point of the format.
  if (lastSuccess?.outputKind === "pdf" && lastSuccess.inspection) {
    if (pdfView === "document") return <DocumentPanel inspection={lastSuccess.inspection} />;
    if (pdfView === "objects") return <ObjectsPanel objects={lastSuccess.inspection.objects} />;
  }

  return (
    <div className="relative h-full min-w-0 overflow-hidden bg-muted/20">
      {lastSuccess?.outputKind === "pdf" ? (
        <div className="absolute inset-0">
          <PdfPreview url={lastSuccess.outputUrl} dimmed={Boolean(error)} />
        </div>
      ) : (
        lastSuccess && (
          <ImageOutput
            url={lastSuccess.outputUrl}
            zoom={zoom}
            dimmed={Boolean(error)}
            overlay={overlay}
            focus={focus}
          />
        )
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
