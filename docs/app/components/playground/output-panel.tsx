import { Loader2Icon } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "~/lib/utils";
import type { PdfInspection, PdfPageStream } from "~/playground/inspect-pdf";
import type { RenderError, RenderSuccess } from "./use-render-worker";

export type PdfView = "preview" | "document" | "stream";

export const PDF_VIEWS: { id: PdfView; label: string }[] = [
  { id: "preview", label: "Preview" },
  { id: "document", label: "Document" },
  { id: "stream", label: "Stream" },
];

export type Zoom = "fit" | "actual";

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

/**
 * The decoded page content streams. One shaped run emits one `BT` block, so the
 * count beside each page is where a font switch splitting words shows up first.
 */
function StreamPanel({ streams }: { streams: PdfPageStream[] }) {
  if (streams.length === 0) {
    return (
      <div className="flex h-full items-center justify-center bg-muted/20 font-mono text-xs text-muted-foreground">
        no readable content stream
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto bg-muted/20 px-4 py-3 font-mono text-xs">
      {streams.map((stream) => (
        <div key={stream.page} className="mb-4 last:mb-0">
          <div className="mb-1 text-muted-foreground">
            page {stream.page} · {stream.textObjects} text objects
          </div>
          <pre className="whitespace-pre-wrap break-all">{stream.operators}</pre>
        </div>
      ))}
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

export function OutputPanel({
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
  if (lastSuccess?.outputKind === "pdf" && lastSuccess.inspection) {
    if (pdfView === "document") return <DocumentPanel inspection={lastSuccess.inspection} />;
    if (pdfView === "stream") return <StreamPanel streams={lastSuccess.inspection.streams} />;
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
