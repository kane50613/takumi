import {
  ChevronRightIcon,
  CornerLeftUpIcon,
  ImageIcon,
  Loader2Icon,
  ScanIcon,
  SquareIcon,
  TypeIcon,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import {
  type KeyboardEvent,
  type MouseEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { cn } from "~/lib/utils";
import {
  ancestorsOf,
  type InspectedNode,
  nodeAt,
  type PaintInspection,
  type Quad,
} from "~/playground/inspect-paint";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "../ui/resizable";
import type { InspectResult } from "./use-render-worker";

const KIND_ICONS: Record<InspectedNode["kind"], LucideIcon> = {
  box: SquareIcon,
  text: TypeIcon,
  image: ImageIcon,
};

/** Rows deeper than this start collapsed. */
const OPEN_DEPTH = 3;

function points(quad: Quad) {
  return quad.map(({ x, y }) => `${x},${y}`).join(" ");
}

function isTyping(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || target.tagName === "INPUT" || target.tagName === "TEXTAREA")
  );
}

/**
 * The inspector's selection, hover, and focus. Selection and focus are held by node key,
 * so they survive a re-run that paints the same markup.
 */
export function useInspectorSelection(inspection: PaintInspection | undefined) {
  const [selectedKey, setSelectedKey] = useState<string>();
  const [focusedKey, setFocusedKey] = useState<string>();
  const [hovered, setHovered] = useState<number>();

  const nodes = inspection?.nodes;
  const indexOf = (key: string | undefined) => {
    const index = nodes?.findIndex((node) => node.key === key) ?? -1;

    return index === -1 ? undefined : index;
  };
  const selected = indexOf(selectedKey);
  const focused = indexOf(focusedKey);

  const select = useCallback(
    (index: number | undefined) =>
      setSelectedKey(index === undefined ? undefined : nodes?.[index]?.key),
    [nodes],
  );
  const focus = useCallback(
    (index: number | undefined) => {
      setFocusedKey(index === undefined ? undefined : nodes?.[index]?.key);
      if (index !== undefined) select(index);
    },
    [nodes, select],
  );

  useEffect(() => setHovered(undefined), [nodes]);

  // Figma's keys: Escape leaves the zoom, then climbs to the parent; Enter descends;
  // Shift+2 zooms to the selection and Shift+1 zooms back out.
  useEffect(() => {
    if (!nodes) return;

    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (isTyping(event.target) || event.metaKey || event.ctrlKey || event.altKey) return;

      const zoomOut = event.key === "Escape" || (event.shiftKey && event.code === "Digit1");
      const child =
        event.key === "Enter" ? nodes.findIndex((node) => node.parent === selected) : -1;

      if (event.shiftKey && event.code === "Digit2" && selected !== undefined) focus(selected);
      else if (zoomOut && focused !== undefined) setFocusedKey(undefined);
      else if (event.key === "Escape" && selected !== undefined) select(nodes[selected]?.parent);
      else if (child !== -1) select(child);
      else return;

      event.preventDefault();
    };

    window.addEventListener("keydown", onKeyDown);

    return () => window.removeEventListener("keydown", onKeyDown);
  }, [nodes, selected, focused, select, focus]);

  return { selected, focused, hovered, setHovered, select, focus };
}

export type InspectorSelection = ReturnType<typeof useInspectorSelection>;

/** The frames drawn over the render: the hovered node, and the selected one with its text runs. */
export function FrameOverlay({
  inspection,
  selection,
  zoomScale,
}: {
  inspection: PaintInspection;
  selection: InspectorSelection;
  /** How far the focus zoom enlarges the render, so labels can stay their own size. */
  zoomScale: number;
}) {
  const { width, height, nodes } = inspection;
  const { selected, hovered, setHovered, select, focus } = selection;
  const selectedNode = selected === undefined ? undefined : nodes[selected];
  const hoveredNode = hovered === undefined || hovered === selected ? undefined : nodes[hovered];
  const svgRef = useRef<SVGSVGElement>(null);
  const [shownWidth, setShownWidth] = useState(width);

  // Strokes are sized in screen pixels. `vector-effect` cannot do it here: it ignores
  // the CSS transform the focus zoom puts on an ancestor.
  useEffect(() => {
    const svg = svgRef.current;

    if (!svg) return;

    const observer = new ResizeObserver(() => setShownWidth(svg.clientWidth || width));

    observer.observe(svg);

    return () => observer.disconnect();
  }, [width]);

  const px = (pixels: number) => (pixels * width) / (shownWidth * zoomScale);

  const pointAt = (event: MouseEvent<SVGSVGElement>) => {
    const box = event.currentTarget.getBoundingClientRect();

    return nodeAt(inspection, {
      x: ((event.clientX - box.left) / box.width) * width,
      y: ((event.clientY - box.top) / box.height) * height,
    });
  };

  return (
    <>
      <svg
        ref={svgRef}
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        role="presentation"
        className="absolute inset-0 size-full overflow-visible text-primary"
        onPointerMove={(event) => setHovered(pointAt(event))}
        onPointerLeave={() => setHovered(undefined)}
        onClick={(event) => select(pointAt(event))}
        onDoubleClick={(event) => focus(pointAt(event))}
      >
        {hoveredNode && (
          <polygon
            points={points(hoveredNode.quad)}
            fill="none"
            stroke="currentColor"
            strokeWidth={px(1)}
          />
        )}
        {selectedNode?.contentQuad && (
          <polygon
            points={points(selectedNode.contentQuad)}
            fill="none"
            stroke="currentColor"
            strokeOpacity={0.6}
            strokeDasharray={`${px(3)} ${px(3)}`}
            strokeWidth={px(1)}
          />
        )}
        {selectedNode?.runQuads?.map((quad, index) => (
          <polygon
            key={index}
            points={points(quad)}
            fill="currentColor"
            fillOpacity={0.12}
            stroke="none"
          />
        ))}
        {selectedNode && (
          <polygon
            points={points(selectedNode.quad)}
            fill="none"
            stroke="currentColor"
            strokeWidth={px(1.5)}
          />
        )}
      </svg>
      {selectedNode && (
        <div
          className="pointer-events-none absolute whitespace-nowrap rounded-sm bg-primary px-1 font-mono text-[10px] leading-4 text-primary-foreground"
          style={{
            left: `${((selectedNode.bounds.x + selectedNode.bounds.width / 2) / width) * 100}%`,
            top: `${((selectedNode.bounds.y + selectedNode.bounds.height) / height) * 100}%`,
            transform: `scale(${1 / zoomScale}) translate(-50%, 4px)`,
            transformOrigin: "top left",
          }}
        >
          {Math.round(selectedNode.bounds.width)} × {Math.round(selectedNode.bounds.height)}
        </div>
      )}
    </>
  );
}

/** Each visible row, skipping the descendants of collapsed rows. */
function visibleRows(nodes: InspectedNode[], isOpen: (node: InspectedNode) => boolean) {
  const rows: number[] = [];
  let hiddenBelow = Number.POSITIVE_INFINITY;

  nodes.forEach((node, index) => {
    if (node.depth > hiddenBelow) return;

    hiddenBelow = node.hasChildren && !isOpen(node) ? node.depth : Number.POSITIVE_INFINITY;
    rows.push(index);
  });

  return rows;
}

function LayerTree({
  inspection,
  selection,
}: {
  inspection: PaintInspection;
  selection: InspectorSelection;
}) {
  const { nodes } = inspection;
  const { selected, hovered, setHovered, select, focus } = selection;
  // Keys whose open state differs from the depth default.
  const [toggled, setToggled] = useState<ReadonlySet<string>>(new Set());
  const listRef = useRef<HTMLDivElement>(null);

  const isOpen = useCallback(
    (node: InspectedNode) => node.depth < OPEN_DEPTH !== toggled.has(node.key),
    [toggled],
  );
  const setOpen = useCallback((targets: InspectedNode[], open: boolean) => {
    setToggled((current) => {
      const next = new Set(current);

      for (const node of targets) {
        if (node.depth < OPEN_DEPTH === open) next.delete(node.key);
        else next.add(node.key);
      }

      return next;
    });
  }, []);

  // A node picked on the render opens every row above it.
  useEffect(() => {
    if (selected === undefined) return;

    const ancestors = ancestorsOf(nodes, selected).flatMap((index) => nodes[index] ?? []);

    if (ancestors.some((node) => !isOpen(node))) setOpen(ancestors, true);
  }, [selected, nodes, isOpen, setOpen]);

  useEffect(() => {
    listRef.current?.querySelector("[aria-selected=true]")?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const rows = useMemo(() => visibleRows(nodes, isOpen), [nodes, isOpen]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const position = selected === undefined ? -1 : rows.indexOf(selected);
    const node = selected === undefined ? undefined : nodes[selected];

    if (event.key === "ArrowDown") select(rows[Math.min(position + 1, rows.length - 1)]);
    else if (event.key === "ArrowUp") select(rows[Math.max(position - 1, 0)]);
    else if (event.key === "ArrowRight" && node?.hasChildren) {
      if (isOpen(node)) select(rows[position + 1]);
      else setOpen([node], true);
    } else if (event.key === "ArrowLeft" && node) {
      if (node.hasChildren && isOpen(node)) setOpen([node], false);
      else select(node.parent);
    } else return;

    event.preventDefault();
  };

  return (
    <div
      ref={listRef}
      role="tree"
      aria-label="Layers"
      tabIndex={0}
      onKeyDown={onKeyDown}
      onPointerLeave={() => setHovered(undefined)}
      className="h-full overflow-auto py-1 font-mono text-xs outline-none"
    >
      {rows.map((index) => {
        const node = nodes[index];

        if (!node) return null;

        const Icon = KIND_ICONS[node.kind];
        const open = isOpen(node);

        return (
          <div
            key={node.key}
            role="treeitem"
            aria-selected={index === selected}
            aria-expanded={node.hasChildren ? open : undefined}
            onPointerEnter={() => setHovered(index)}
            onClick={() => select(index)}
            onDoubleClick={() => focus(index)}
            style={{ paddingLeft: 4 + node.depth * 12 }}
            className={cn(
              "flex h-6 cursor-default items-center gap-1 pr-2",
              index === selected
                ? "bg-primary/15 text-foreground"
                : index === hovered
                  ? "bg-muted text-foreground"
                  : "text-muted-foreground",
            )}
          >
            {node.hasChildren ? (
              <button
                type="button"
                tabIndex={-1}
                aria-label={open ? "Collapse" : "Expand"}
                onClick={(event) => {
                  event.stopPropagation();
                  setOpen([node], !open);
                }}
                className="flex size-4 shrink-0 items-center justify-center"
              >
                <ChevronRightIcon className={cn("size-3", open && "rotate-90")} />
              </button>
            ) : (
              <span className="size-4 shrink-0" />
            )}
            <Icon className="size-3 shrink-0" />
            <span className={cn("truncate text-foreground", node.detail && "shrink-0")}>
              {node.name}
            </span>
            {node.detail && <span className="min-w-0 truncate">{node.detail}</span>}
          </div>
        );
      })}
      {inspection.omitted > 0 && (
        <div className="px-3 py-1 text-muted-foreground">
          {inspection.omitted} more nodes left out
        </div>
      )}
    </div>
  );
}

function Properties({
  inspection,
  selection,
}: {
  inspection: PaintInspection;
  selection: InspectorSelection;
}) {
  const { selected, select, focus } = selection;
  const node = selected === undefined ? undefined : inspection.nodes[selected];

  if (!node) {
    return (
      <div className="flex h-full items-center justify-center p-4 text-center font-mono text-xs text-muted-foreground">
        Click the render or a layer to inspect it.
      </div>
    );
  }

  const Icon = KIND_ICONS[node.kind];

  return (
    <div className="h-full overflow-auto px-3 py-2 font-mono text-xs">
      <div className="flex items-center gap-1.5">
        <Icon className="size-3 shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 truncate">{node.name}</span>
        <button
          type="button"
          title="Select parent (Esc)"
          disabled={node.parent === undefined}
          onClick={() => select(node.parent)}
          className="rounded-sm p-1 text-muted-foreground transition-colors hover:text-foreground disabled:opacity-40"
        >
          <CornerLeftUpIcon className="size-3" />
        </button>
        <button
          type="button"
          title="Zoom to layer (⇧2)"
          onClick={() => focus(selected)}
          className="rounded-sm p-1 text-muted-foreground transition-colors hover:text-foreground"
        >
          <ScanIcon className="size-3" />
        </button>
      </div>
      {node.sections.map((section) => (
        <section key={section.title} className="mt-4">
          <h3 className="mb-1 text-[10px] uppercase tracking-wide text-muted-foreground">
            {section.title}
          </h3>
          {section.rows.map((row, index) => (
            <div key={index} className="flex gap-3 py-0.5">
              <span className="w-20 shrink-0 truncate text-muted-foreground">{row.label}</span>
              <span className="flex min-w-0 flex-1 items-start gap-1.5">
                {row.swatch && (
                  <span
                    className="mt-0.5 size-3 shrink-0 rounded-sm border"
                    style={{ background: row.swatch }}
                  />
                )}
                <span className="break-words">{row.value}</span>
              </span>
            </div>
          ))}
        </section>
      ))}
    </div>
  );
}

/** The layers and properties of the last image render, read from its paint tree. */
export function InspectorPanel({
  result,
  selection,
}: {
  result: InspectResult | undefined;
  selection: InspectorSelection;
}) {
  if (!result) {
    return (
      <div className="flex h-full items-center justify-center gap-2 bg-muted/20 font-mono text-xs text-muted-foreground">
        <Loader2Icon className="size-3.5 animate-spin" />
        painting…
      </div>
    );
  }

  if (result.status === "error") {
    return (
      <div className="h-full overflow-auto bg-muted/20 px-3 py-2 font-mono text-xs">
        <pre className="whitespace-pre-wrap text-muted-foreground">{result.message}</pre>
      </div>
    );
  }

  return (
    <ResizablePanelGroup orientation="horizontal">
      <ResizablePanel defaultSize={50} minSize={25}>
        <LayerTree inspection={result.inspection} selection={selection} />
      </ResizablePanel>
      <ResizableHandle className="hover:bg-primary/50 transition-colors" />
      <ResizablePanel defaultSize={50} minSize={25}>
        <Properties inspection={result.inspection} selection={selection} />
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
