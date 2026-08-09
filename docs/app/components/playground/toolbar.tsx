import {
  CheckIcon,
  ChevronDownIcon,
  Code2Icon,
  DownloadIcon,
  EyeIcon,
  FileTextIcon,
  FilmIcon,
  ImageIcon,
  LinkIcon,
  Loader2Icon,
  PlayIcon,
  RotateCcwIcon,
  Wand2Icon,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { cn } from "~/lib/utils";
import { type OutputKind, outputKinds } from "~/playground/schema";
import { type Template, templates } from "~/playground/templates";
import { Button } from "../ui/button";
import { Kbd } from "../ui/kbd";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import type { Zoom } from "./output-panel";

export type TabId = "code" | "preview";

const TABS: { id: TabId; label: string; icon: LucideIcon }[] = [
  { id: "code", label: "Code", icon: Code2Icon },
  { id: "preview", label: "Preview", icon: EyeIcon },
];

const KINDS: Record<OutputKind, { label: string; icon: LucideIcon }> = {
  image: { label: "Images", icon: ImageIcon },
  animation: { label: "Animations", icon: FilmIcon },
  pdf: { label: "Documents", icon: FileTextIcon },
};

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

export function Toolbar({
  activeTab,
  setActiveTab,
  selectedTemplateName,
  selectedTemplateId,
  onSelectTemplate,
  isStale,
  isReady,
  onRun,
  hintDismissed,
  onDismissHint,
  isFormatting,
  onFormat,
  onReset,
  outputKind,
  zoom,
  onToggleZoom,
  copied,
  onCopyShareLink,
  hasOutput,
  onDownload,
}: {
  activeTab: TabId;
  setActiveTab: (tab: TabId) => void;
  selectedTemplateName: string;
  selectedTemplateId: string | undefined;
  onSelectTemplate: (template: Template) => void;
  isStale: boolean;
  isReady: boolean;
  onRun: () => void;
  hintDismissed: boolean;
  onDismissHint: () => void;
  isFormatting: boolean;
  onFormat: () => void;
  onReset: () => void;
  outputKind: OutputKind | undefined;
  zoom: Zoom;
  onToggleZoom: () => void;
  copied: boolean;
  onCopyShareLink: () => void;
  hasOutput: boolean;
  onDownload: () => void;
}) {
  return (
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
        selectedId={selectedTemplateId}
        onSelect={onSelectTemplate}
      />

      <div className="relative flex shrink-0 items-center gap-0.5">
        <Button
          variant={isStale ? "default" : "ghost"}
          size="sm"
          className="h-7 gap-1.5 px-2 font-mono text-xs"
          onClick={onRun}
          disabled={!isStale || !isReady}
          title="Run the code (⌘↵)"
        >
          <PlayIcon className="size-3.5" />
          <span className="max-md:hidden">Run</span>
          <Kbd className="ml-1 bg-current/15 px-1.5 text-[11px] text-current max-md:hidden">⌘↵</Kbd>
        </Button>
        {isStale && !hintDismissed && (
          <div className="absolute top-9 left-0 z-20 w-56 rounded-md border bg-popover p-3 text-xs shadow-md">
            <p className="m-0 text-popover-foreground">
              The preview waits for you. Hit Run, or press ⌘↵, to render what you just wrote.
            </p>
            <button
              type="button"
              onClick={onDismissHint}
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
          onClick={onFormat}
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
          onClick={onReset}
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
            onClick={onToggleZoom}
            title="Toggle zoom"
          >
            {zoom === "fit" ? "Fit" : "100%"}
          </Button>
        )}
        <Button
          variant="ghost"
          size="sm"
          className="h-7 gap-1.5 px-2 font-mono text-xs text-muted-foreground"
          onClick={onCopyShareLink}
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
          onClick={onDownload}
          disabled={!hasOutput}
          title="Download output"
        >
          <DownloadIcon className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}
