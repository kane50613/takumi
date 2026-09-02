import type { CssInput } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import type { FontLoader, ImagesInput, RegisteredFamilyLike } from "@takumi-rs/helpers/renderer";
import { FontRegistry } from "@takumi-rs/helpers/renderer";
import {
  LIST_MARKER_CHARACTERS,
  type Node,
  type ReactElementLike,
  subsetFonts,
} from "@takumi-rs/helpers";
import type { ReactNode } from "react";
import { counterCharacters, PdfRenderer as PdfRendererInternal } from "../pkg/takumi_pdf_wasm";

export { default, initSync } from "../pkg/takumi_pdf_wasm";
export type { FontLoader, ImagesInput } from "@takumi-rs/helpers/renderer";
export { PageNumber, TargetPageNumber, TotalPages } from "./primitives";
export type { CounterProps, CounterStyle } from "./primitives";

/** Every class name in a tree, so the renderer can say what its counters draw. */
function classNames(node: unknown): string[] {
  const into: string[] = [];

  collectClassNames(node, into);
  return into;
}

function collectClassNames(node: unknown, into: string[]): void {
  if (typeof node !== "object" || node === null) {
    return;
  }
  const { className, children } = node as { className?: unknown; children?: unknown };

  if (typeof className === "string") {
    into.push(...className.split(/\s+/).filter(Boolean));
  }
  if (Array.isArray(children)) {
    for (const child of children) {
      collectClassNames(child, into);
    }
  }
}

/** A document input: a takumi node tree, JSX, or an HTML string. */
export type NodeInput = Node | ReactNode | ReactElementLike | string;

/** Explicit dimensions in CSS px (96 dpi). */
export type Dimensions = { width: number; height: number };

/** A single-page viewport. Omitting `height` sizes the page to the content. */
export type ViewportInput = { width: number; height?: number };

/** A page keyword CSS Paged Media defines, matched case-insensitively. Portrait. */
export type PageSizeName =
  | "a3"
  | "a4"
  | "a5"
  | "b4"
  | "b5"
  | "jis-b4"
  | "jis-b5"
  | "ledger"
  | "legal"
  | "letter";

/** A page size: a preset name or explicit {@link Dimensions}. */
export type PageSize = PageSizeName | Dimensions;

/**
 * One side of a page margin: a length in CSS px, or `"auto"` to fit the band
 * that draws on that side. `"auto"` never goes below the 37.8 a page starts with, and the
 * left and right sides hold no band, so they land on it.
 */
export type PageMarginSide = number | "auto";

/** A page margin: one value for all sides, or per-side values (a side left out is `auto`). */
export type PageMargin =
  | PageMarginSide
  | {
      top?: PageMarginSide;
      right?: PageMarginSide;
      bottom?: PageMarginSide;
      left?: PageMarginSide;
    };

/**
 * A page to keep: a 1-based page number, or an inclusive span. An unset `from`
 * starts at the first page; an unset `to` runs to the last.
 */
export type PageRange = number | { from?: number; to?: number };

/**
 * Paged output (the default): content flows across pages of `size`, like
 * Puppeteer's `page.pdf()`. The layout canvas has unbounded height, so
 * percentage heights do not resolve. CSS `@page` rules in `css` are
 * not supported — page geometry comes from these options.
 */
type PagedOptions = {
  viewport?: never;
  /** Page size. Defaults to A4. */
  size?: PageSize;
  /** Swaps the page's width and height, including explicit sizes. */
  landscape?: boolean;
  /** Page margin. Defaults to `"auto"` on every side. */
  margin?: PageMargin;
  /**
   * Band repeated at the top of every page. Nodes with the `pageNumber` or
   * `totalPages` class receive the counter as text, like Chromium's print
   * templates; add a CSS `@counter-style` name (e.g. `cjk-decimal`,
   * `lower-roman`) to the class list to format it.
   */
  header?: NodeInput;
  /** Band repeated at the bottom of every page; same class hooks as `header`. */
  footer?: NodeInput;
  /**
   * The pages the output keeps, e.g. `[1, { from: 4, to: 8 }]`, like a print
   * dialog's page ranges. Layout and page counters still run over the whole
   * document, so a kept page shows the numbers it would in full output.
   * Ranges that keep no page reject the render.
   */
  pageRanges?: PageRange[];
};

/**
 * Single-page output: a fixed viewport, like an image render. Percentage
 * heights resolve against it and overflowing content is clipped. Omitting
 * `height` instead sizes the single page to the laid-out content, like a
 * thermal receipt; percentage heights do not resolve there.
 */
type ViewportOptions = {
  viewport: ViewportInput;
  size?: never;
  landscape?: never;
  margin?: never;
  header?: never;
  footer?: never;
  pageRanges?: never;
};

/**
 * Options for {@link PdfRenderer.measure}: page geometry (or a viewport) plus
 * layout resources. Margins do not affect the result.
 */
export type MeasureOptions = (
  | { size?: PageSize; landscape?: boolean; viewport?: never }
  | { viewport: ViewportInput; size?: never; landscape?: never }
) & {
  /** Fonts to register before layout, deduped across calls. */
  fonts?: FontLoader[];
  /** Pre-fetched images for `src` URLs in the tree. */
  images?: ImagesInput;
  /** CSS to apply before layout, one string or a list cascading in order. */
  css?: CssInput | readonly CssInput[];
  /**
   * CSS stylesheets to apply before layout.
   * @deprecated Use `css` instead. Will be removed in v3.
   */
  stylesheets?: string[];
  /** Per-render font stack: ordered family names used as the fallback chain. */
  fontFamilies?: string[];
  /** Default BCP-47 language tag applied to the root. */
  lang?: string;
};

/** A node tree's laid-out size in CSS px. */
export type MeasuredSize = { width: number; height: number };

/** Document metadata written to the PDF's info dictionary. */
export type PdfMetadata = {
  /** The document title. */
  title?: string;
  /** The document description (the info dictionary's subject). */
  description?: string;
  /** The document authors. */
  authors?: string[];
  /** The document keywords. */
  keywords?: string[];
  /** The tool that created the source document. */
  creator?: string;
  /**
   * UTC creation date, `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`. Tagged archival
   * standards require one; supplying it keeps output deterministic.
   */
  creationDate?: string;
  /**
   * Custom XMP schemas written into the packet, for metadata the renderer
   * knows nothing about, e.g. the `fx:` properties a Factur-X invoice needs.
   */
  xmp?: XmpSchema[];
};

/**
 * A namespace written into the XMP packet, with the schema description PDF/A
 * requires for it.
 */
export type XmpSchema = {
  /** Human-readable name, e.g. "Factur-X PDF/A Extension". */
  name: string;
  /** Namespace prefix the properties are written under, e.g. "fx". */
  prefix: string;
  /** Namespace URI. */
  namespace: string;
  /**
   * Properties written under the namespace. Each is written as a value and
   * described in the schema, so the two cannot drift apart.
   */
  properties: XmpProperty[];
};

/** A property of an {@link XmpSchema}. */
export type XmpProperty = {
  /** Property name, e.g. "DocumentFileName". */
  name: string;
  /** Property value. */
  value: string;
  /** What the property means. PDF/A requires one. */
  description: string;
};

/** A file attached to the PDF, shown in the viewer's attachment panel. */
export type Attachment = {
  /** File name in the PDF, e.g. "factur-x.xml". */
  name: string;
  /** The file's bytes, or a string encoded as UTF-8. */
  data: Uint8Array | string;
  /** IANA media type, e.g. "application/xml". The PDF/A-3 levels require one. */
  mimeType?: string;
  /** Human-readable description. The PDF/A-3 levels require one. */
  description?: string;
  /**
   * How the file relates to the document (the PDF/A-3 AFRelationship).
   * Defaults to "unspecified".
   */
  relationship?: "source" | "data" | "alternative" | "supplement" | "unspecified";
  /**
   * UTC modification date, `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`; falls back
   * to `metadata.creationDate`. The PDF/A-3 levels require one.
   */
  modificationDate?: string;
};

/** An attachment under the PDF/A-3 levels, which require the descriptive fields. */
export type ArchivalAttachment = Attachment & {
  mimeType: string;
  description: string;
};

/**
 * Standards conformance. Invalid combinations are type errors: the `a` levels
 * imply a structure tree so `tagged: false` is rejected, PDF/UA-1 is PDF
 * 1.7-only and PDF/UA-2 is PDF 2.0-only so each pairs with its own PDF/A
 * levels, and only the PDF/A-3 levels, PDF/A-4f (or plain PDF) accept
 * attachments.
 */
type ConformanceOptions =
  | {
      pdfa?: never;
      /** Structure tree: off, on (default), or validated against PDF/UA. */
      tagged?: boolean | "ua1" | "ua2";
      /** Files attached to the document. */
      attachments?: Attachment[];
    }
  | {
      /** PDF/A conformance level. Validation failures reject the render. */
      pdfa: "2b" | "2u";
      tagged?: boolean | "ua1";
      attachments?: never;
    }
  | { pdfa: "2a"; tagged?: true | "ua1"; attachments?: never }
  | { pdfa: "3b" | "3u"; tagged?: boolean | "ua1"; attachments?: ArchivalAttachment[] }
  | { pdfa: "3a"; tagged?: true | "ua1"; attachments?: ArchivalAttachment[] }
  | { pdfa: "4"; tagged?: boolean | "ua2"; attachments?: never }
  | { pdfa: "4f"; tagged?: boolean | "ua2"; attachments?: ArchivalAttachment[] };

/** How the document's fillable fields are emitted. */
export type FormOptions = {
  /**
   * Family embedded into the form's default resources, which a viewer redraws
   * an edited field with. Defaults to the first registered family.
   */
  font?: string;
};

export type RenderOptions = (PagedOptions | ViewportOptions) &
  ConformanceOptions & {
    /** Fonts to register before rendering, deduped across calls. */
    fonts?: FontLoader[];
    /**
     * Pre-fetched images for `src` URLs in the tree, e.g.
     * `[{ src: "https://…/logo.png", data: bytes }]`.
     */
    images?: ImagesInput;
    /** CSS to apply before layout, one string or a list cascading in order. */
    css?: CssInput | readonly CssInput[];
    /**
     * CSS stylesheets to apply before layout.
     * @deprecated Use `css` instead. Will be removed in v3.
     */
    stylesheets?: string[];
    /** CSS custom properties for `:root`; the `--` prefix is optional. */
    /** Per-render font stack: ordered family names used as the fallback chain. */
    fontFamilies?: string[];
    /** Default BCP-47 language tag applied to the root. */
    lang?: string;
    /** Document metadata; `lang` doubles as the metadata language. */
    metadata?: PdfMetadata;
    /** Generates a PDF outline (bookmarks) from `h1`–`h6` headings. */
    outline?: boolean;
    /**
     * Emits `<input>`, `<textarea>` and `<select>` as fillable AcroForm
     * fields. Left unset they draw as the static boxes their CSS describes.
     */
    form?: boolean | FormOptions;
    /**
     * The paper color as a CSS color, painted under everything on every page.
     * Unset leaves the page empty, so a viewer shows its own white.
     */
    backgroundColor?: string;
  };

function isNode(value: NodeInput): value is Node {
  return typeof value === "object" && value !== null && "type" in value && !("$$typeof" in value);
}

let warnedStylesheets = false;

function warnStylesheetsDeprecated(): void {
  if (warnedStylesheets) return;
  warnedStylesheets = true;
  console.warn("takumi: the `stylesheets` option is deprecated, use `css` instead.");
}

/** Narrows the `css` option, which takes one entry or a list of them. */
function isCssList(css: CssInput | readonly CssInput[]): css is readonly CssInput[] {
  return Array.isArray(css);
}

function ownCss(
  css: CssInput | readonly CssInput[] | undefined,
  stylesheets: string[] | undefined,
): CssInput[] {
  if (css !== undefined && stylesheets !== undefined) {
    throw new Error("pass either `css` or `stylesheets`, not both");
  }

  if (css !== undefined) {
    return isCssList(css) ? [...css] : [css];
  }

  if (stylesheets !== undefined) {
    warnStylesheetsDeprecated();
  }

  return stylesheets ?? [];
}

async function resolveNode(input: NodeInput): Promise<{ node: Node; css: string[] }> {
  if (isNode(input)) {
    return { node: input, css: [] };
  }

  if (typeof input === "string") {
    return fromHtml(input);
  }

  return fromJsx(input as ReactNode);
}

/** A PDF renderer holding registered fonts. Reuse one instance across renders. */
export class PdfRenderer {
  private inner = new PdfRendererInternal();
  private fonts = new FontRegistry<RegisteredFamilyLike>(
    (font) => this.inner.registerFont(font) as RegisteredFamilyLike[],
  );

  /** Renders a node tree, JSX, or an HTML string to PDF bytes. See {@link RenderOptions}. */
  async render(node: NodeInput, options: RenderOptions = {}): Promise<Uint8Array> {
    const { fonts, images, header, footer, css, stylesheets, fontFamilies, ...rest } = options;
    const [main, headerResult, footerResult] = await Promise.all([
      resolveNode(node),
      header === undefined ? undefined : resolveNode(header),
      footer === undefined ? undefined : resolveNode(footer),
    ]);
    const bands = [headerResult?.node, footerResult?.node].filter((band) => band !== undefined);
    const resources = await this.fonts.resolveResources(
      fonts &&
        subsetFonts({
          fonts,
          // Page counters and list markers render characters no node in the
          // tree carries, so the renderer says which ones.
          source: [
            main.node,
            ...bands,
            LIST_MARKER_CHARACTERS,
            counterCharacters([main.node, ...bands].flatMap((tree) => classNames(tree))),
          ],
        }),
      images,
      fontFamilies,
    );
    const sheets = [
      ...ownCss(css, stylesheets),
      ...main.css,
      ...(headerResult?.css ?? []),
      ...(footerResult?.css ?? []),
    ];

    return this.inner.render(main.node, {
      ...rest,
      header: headerResult?.node,
      footer: footerResult?.node,
      css: sheets.length > 0 ? sheets : undefined,
      images: resources.images,
      fontFamilies: resources.fontFamilies,
    });
  }

  /**
   * Lays out a node tree without rendering and returns its size in CSS px.
   *
   * With page options the tree lays out against the full page width with
   * unbounded height, exactly how {@link render} measures a header or footer
   * band (`pageNumber` / `totalPages` hooks are filled with three-digit
   * counters), so the height tells you how much margin a band needs.
   *
   * The returned size is the tree's own, not the space it was laid out
   * against: a box with `width: 100px` measures 100 wide on any page.
   */
  async measure(node: NodeInput, options: MeasureOptions = {}): Promise<MeasuredSize> {
    const { fonts, images, css, stylesheets, fontFamilies, ...rest } = options;
    const main = await resolveNode(node);
    // The tree measured may itself be a band, so its counters are always in play.
    const resources = await this.fonts.resolveResources(
      fonts &&
        subsetFonts({
          fonts,
          source: [main.node, LIST_MARKER_CHARACTERS, counterCharacters(classNames(main.node))],
        }),
      images,
      fontFamilies,
    );
    const sheets = [...ownCss(css, stylesheets), ...main.css];

    return this.inner.measure(main.node, {
      ...rest,
      css: sheets.length > 0 ? sheets : undefined,
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

/** Measures with a lazily created shared {@link PdfRenderer}. */
export function measure(node: NodeInput, options?: MeasureOptions): Promise<MeasuredSize> {
  shared ??= new PdfRenderer();
  return shared.measure(node, options);
}
