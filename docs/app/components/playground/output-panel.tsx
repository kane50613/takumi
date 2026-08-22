import { Loader2Icon } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { type ReactNode, useState } from "react";
import { cn } from "~/lib/utils";
import type { PdfInspection, PdfObject } from "~/playground/inspect-pdf";
import type { RenderError, RenderSuccess } from "./use-render-worker";

export type PdfView = "preview" | "document" | "objects";

export const PDF_VIEWS: { id: PdfView; label: string }[] = [
  { id: "preview", label: "Preview" },
  { id: "document", label: "Document" },
  { id: "objects", label: "Objects" },
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
        <span className={isReady ? undefined : "playground-breathe"}>
          {isReady ? "rendering…" : "loading wasm…"}
        </span>
      </div>
    );
  }

  // The browser's own PDF viewer brings paging, zoom and text selection, which
  // is the point of the format.
  if (lastSuccess?.outputKind === "pdf" && lastSuccess.inspection) {
    if (pdfView === "document") return <DocumentPanel inspection={lastSuccess.inspection} />;
    if (pdfView === "objects") return <ObjectsPanel objects={lastSuccess.inspection.objects} />;
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
