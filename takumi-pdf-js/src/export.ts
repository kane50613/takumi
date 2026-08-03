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

/** Paged output geometry. The preset supplies the size (default `a4`), explicit fields override. */
export type PageInput = {
  preset?: "a4" | "letter";
  landscape?: boolean;
  /** Page width in CSS px (96 dpi). */
  width?: number;
  /** Page height in CSS px (96 dpi). */
  height?: number;
  /** Uniform margin in CSS px. Presets default to half an inch. */
  margin?: number;
};

export type RenderOptions = {
  /** Viewport width for single-page output. Requires `height`; ignored when `page` is set. */
  width?: number;
  /** Viewport height for single-page output. */
  height?: number;
  /**
   * Paged output. A preset name or a {@link PageInput}. Omitting `page`,
   * `width`, and `height` entirely renders paged A4.
   */
  page?: PageInput | "a4" | "letter";
  /** Band repeated at the top of every page. Text may use `{page}` and `{pages}`. */
  header?: NodeInput;
  /** Band repeated at the bottom of every page; same placeholders as `header`. */
  footer?: NodeInput;
  /** Fonts to register before rendering, deduped across calls. */
  fonts?: FontLoader[];
  /** Pre-fetched images keyed by URL. */
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
    const { fonts, images, header, footer, page, stylesheets, fontFamilies, ...rest } = options;
    const sheets = [...(stylesheets ?? [])];
    const resolved = await resolveNode(node, sheets);
    const headerNode = header === undefined ? undefined : await resolveNode(header, sheets);
    const footerNode = footer === undefined ? undefined : await resolveNode(footer, sheets);
    const resources = await this.fonts.resolveResources(fonts, images, fontFamilies);

    return this.inner.render(resolved, {
      ...rest,
      page: typeof page === "string" ? { preset: page } : page,
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
