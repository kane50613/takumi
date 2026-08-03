import { fromJsx } from "@takumi-rs/helpers/jsx";
import type { FontLoader, ImagesInput, RegisteredFamilyLike } from "@takumi-rs/helpers/renderer";
import { FontRegistry } from "@takumi-rs/helpers/renderer";
import type { Node, ReactElementLike } from "@takumi-rs/helpers";
import type { ReactNode } from "react";
import { PdfRenderer as PdfRendererInternal } from "../pkg/takumi_pdf_wasm";

export { default, initSync } from "../pkg/takumi_pdf_wasm";
export type { FontLoader, ImagesInput } from "@takumi-rs/helpers/renderer";

/** A document input: a takumi node tree or JSX. */
export type NodeInput = Node | ReactNode | ReactElementLike;

/** Explicit dimensions in CSS px (96 dpi). */
export type Dimensions = { width: number; height: number };

/** A page size: a preset name (case-insensitive) or explicit {@link Dimensions}. */
export type PageSize = "a4" | "letter" | Dimensions;

/** A page margin in CSS px: one number for all sides, or per-side values (missing sides are zero). */
export type PageMargin = number | { top?: number; right?: number; bottom?: number; left?: number };

/**
 * Paged output (the default): content flows across pages of `size`, like
 * Puppeteer's `page.pdf()`. The layout canvas has unbounded height, so
 * percentage heights do not resolve. CSS `@page` rules in `stylesheets` are
 * not supported — page geometry comes from these options.
 */
type PagedOptions = {
  viewport?: never;
  /** Page size. Defaults to A4. */
  size?: PageSize;
  /** Swaps the page's width and height. */
  landscape?: boolean;
  /** Page margin. Defaults to a uniform 48 (half an inch). */
  margin?: PageMargin;
  /** Band repeated at the top of every page. Text may use `{page}` and `{pages}`. */
  header?: NodeInput;
  /** Band repeated at the bottom of every page; same placeholders as `header`. */
  footer?: NodeInput;
};

/**
 * Single-page output: a fixed viewport, like an image render. Percentage
 * heights resolve against it and overflowing content is clipped.
 */
type ViewportOptions = {
  viewport: Dimensions;
  size?: never;
  landscape?: never;
  margin?: never;
  header?: never;
  footer?: never;
};

export type RenderOptions = (PagedOptions | ViewportOptions) & {
  /** Fonts to register before rendering, deduped across calls. */
  fonts?: FontLoader[];
  /**
   * Pre-fetched images for `src` URLs in the tree, e.g.
   * `[{ src: "https://…/logo.png", data: bytes }]`.
   */
  images?: ImagesInput;
  /** CSS stylesheets to apply before layout. */
  stylesheets?: string[];
  /** Per-render font stack: ordered family names used as the fallback chain. */
  fontFamilies?: string[];
  /** Default BCP-47 language tag applied to the root. */
  lang?: string;
};

function isNode(value: NodeInput): value is Node {
  return typeof value === "object" && value !== null && "type" in value && !("$$typeof" in value);
}

async function resolveNode(input: NodeInput, stylesheets: string[]): Promise<Node> {
  if (isNode(input)) {
    return input;
  }
  const result = await fromJsx(input as ReactNode);

  stylesheets.push(...result.stylesheets);
  return result.node;
}

/** A PDF renderer holding registered fonts. Reuse one instance across renders. */
export class PdfRenderer {
  private inner = new PdfRendererInternal();
  private fonts = new FontRegistry<RegisteredFamilyLike>(
    (font) => this.inner.registerFont(font) as RegisteredFamilyLike[],
  );

  /** Renders a node tree or JSX to PDF bytes. See {@link RenderOptions}. */
  async render(node: NodeInput, options: RenderOptions = {}): Promise<Uint8Array> {
    const { fonts, images, header, footer, stylesheets, fontFamilies, ...rest } = options;
    const sheets = [...(stylesheets ?? [])];
    const resolved = await resolveNode(node, sheets);
    const headerNode = header === undefined ? undefined : await resolveNode(header, sheets);
    const footerNode = footer === undefined ? undefined : await resolveNode(footer, sheets);
    const resources = await this.fonts.resolveResources(fonts, images, fontFamilies);

    return this.inner.render(resolved, {
      ...rest,
      header: headerNode,
      footer: footerNode,
      stylesheets: sheets.length > 0 ? sheets : undefined,
      images: resources.images,
      fontFamilies: resources.fontFamilies,
    });
  }

  /** Registers a font ahead of time, deduped against earlier registrations. */
  registerFont(font: FontLoader) {
    return this.fonts.register(font);
  }

  /** Releases the underlying wasm renderer's memory. */
  free() {
    this.inner.free();
  }
}

let shared: PdfRenderer | undefined;

/** Renders with a lazily created shared {@link PdfRenderer}. */
export function render(node: NodeInput, options?: RenderOptions): Promise<Uint8Array> {
  shared ??= new PdfRenderer();
  return shared.render(node, options);
}
