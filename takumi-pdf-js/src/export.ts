import type { CssInput } from "@takumi-rs/helpers";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import type {
  FontLoader,
  ImageSource,
  ImagesInput,
  PrepareImagesInput,
  RegisteredFamilyLike,
  SourceNeeds,
} from "@takumi-rs/helpers/renderer";
import {
  FontRegistry,
  joinNeeds,
  type JsonSource,
  Prepared,
  prepareSource,
  readSource,
  renderPrepared,
  toNativeSource,
} from "@takumi-rs/helpers/renderer";
import type { Node, ReactElementLike } from "@takumi-rs/helpers";
import type { ReactNode } from "react";
import {
  type FontCacheOptions,
  type PageOverride,
  type PageRules,
  PdfRenderer as PdfRendererInternal,
  type RegisteredFamily,
} from "../pkg/takumi_pdf_wasm";

export { default, initSync } from "../pkg/takumi_pdf_wasm";
export type { FontCacheOptions } from "../pkg/takumi_pdf_wasm";
export type { FontLoader, ImagesInput, PrepareImagesInput } from "@takumi-rs/helpers/renderer";
export { Prepared } from "@takumi-rs/helpers/renderer";
export { PageNumber, TargetPageNumber, TotalPages } from "./primitives";
export type { CounterProps, CounterStyle } from "./primitives";

/** A document input: a takumi node tree, JSX, or an HTML string. */
export type NodeInput = Node | ReactNode | ReactElementLike | string;

/** A band on a page: a document input, or `false` for none. */
export type BandInput = NodeInput | false;

/** What some pages draw instead of the document's own bands. A field left out falls through. */
export type PageOverrideInput = {
  header?: BandInput;
  footer?: BandInput;
};

/** Overrides keyed by the pages they cover. `first` and `last` win over `odd` and `even`. */
export type PageRulesInput = {
  first?: PageOverrideInput;
  last?: PageOverrideInput;
  odd?: PageOverrideInput;
  even?: PageOverrideInput;
};

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

/** What a character no registered font covers turns into on the page. */
export type UncoveredText = "error" | "placeholder" | "blank";

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
  header?: BandInput;
  /** Band repeated at the bottom of every page; same class hooks as `header`. */
  footer?: BandInput;
  /** What some pages draw differently from the rest. */
  pages?: PageRulesInput;
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
  pages?: never;
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
   * UTC creation date, `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`. PDF/A output
   * requires one when `metadata` is set, or `lang` on tagged output. Supplying
   * it keeps output deterministic.
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
    /** Per-render font stack: ordered family names used as the fallback chain. */
    fontFamilies?: string[];
    /** Default BCP-47 language tag applied to the root. */
    lang?: string;
    /** Document metadata; `lang` doubles as the metadata language. */
    metadata?: PdfMetadata;
    /** Generates a PDF outline (bookmarks) from `h1`–`h6` headings. */
    outline?: boolean;
    /**
     * The paper color as a CSS color, painted under everything on every page.
     * Unset leaves the page empty, so a viewer shows its own white.
     */
    backgroundColor?: string;
    /**
     * What a character no registered font covers turns into: `"error"` (default)
     * fails the render naming it, `"placeholder"` draws the font's glyph 0, and
     * `"blank"` draws nothing. Neither of the last two reflows the line.
     *
     * `"placeholder"` is rejected with any `pdfa` level or `tagged: "ua1"` /
     * `"ua2"`, which forbid glyph 0.
     */
    uncoveredText?: UncoveredText;
    /** Stops the render before it reads the document or draws it. */
    signal?: AbortSignal;
  };

/** The render options that only change how laid-out pages are written to the file. */
type OutputOnly =
  | "metadata"
  | "outline"
  | "backgroundColor"
  | "uncoveredText"
  | "pdfa"
  | "tagged"
  | "attachments";

/** `Omit` that distributes over unions, so the page and conformance variants survive. */
type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never;

/** Options for {@link PdfRenderer.prepare}: everything {@link RenderOptions} lays out with. */
export type PrepareOptions = DistributiveOmit<RenderOptions, OutputOnly | "images"> & {
  /** Images to use as they are, and how to fetch the rest the document draws. */
  images?: PrepareImagesInput;
  signal?: AbortSignal;
};

/** Options for rendering a {@link PreparedSource}: how its pages are written to the file. */
export type PreparedRenderOptions = ConformanceOptions &
  Pick<
    PagedOptions & RenderOptions,
    "metadata" | "outline" | "backgroundColor" | "uncoveredText"
  > & {
    signal?: AbortSignal;
  };

/** A document {@link PdfRenderer.prepare} readied, with its fonts and images loaded. */
export type PreparedSource = Prepared<Node | string | JsonSource, DocumentLayout>;

/** The binding options a document lays out with, its bands resolved to what the wasm side reads. */
type DocumentLayout = DistributiveOmit<
  PrepareOptions,
  "fonts" | "images" | "signal" | "fontFamilies" | "css" | "header" | "footer" | "pages"
> & {
  header?: Node | string;
  footer?: Node | string;
  pages?: PageRules;
  css?: CssInput[];
};

function isNode(value: NodeInput): value is Node {
  return typeof value === "object" && value !== null && "type" in value && !("$$typeof" in value);
}

/** Narrows the `css` option, which takes one entry or a list of them. */
function isCssList(css: CssInput | readonly CssInput[]): css is readonly CssInput[] {
  return Array.isArray(css);
}

function ownCss(css: CssInput | readonly CssInput[] | undefined): CssInput[] {
  return css === undefined ? [] : isCssList(css) ? [...css] : [css];
}

const PAGE_RULES = ["first", "last", "odd", "even"] as const;
const PAGE_OVERRIDE_FIELDS = ["header", "footer"] as const;

/** Page rules with every document input resolved to a node tree. */
type ResolvedRules = { pages: PageRules; nodes: (Node | string)[]; css: string[] };

function rejectNonObject(value: unknown, what: string): void {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError(`${what} takes an object`);
  }
}

function rejectUnknownKeys(object: object, known: readonly string[], what: string): void {
  const unknown = Object.keys(object).filter((key) => !known.includes(key));

  if (unknown.length > 0) {
    throw new TypeError(`unknown ${what}: ${unknown.join(", ")}`);
  }
}

async function resolveRules(input: PageRulesInput): Promise<ResolvedRules> {
  const resolved: ResolvedRules = { pages: {}, nodes: [], css: [] };

  rejectNonObject(input, "pages");
  rejectUnknownKeys(input, PAGE_RULES, "page rule");
  for (const rule of PAGE_RULES) {
    const override = input[rule];

    if (override == null) continue;
    rejectNonObject(override, `pages.${rule}`);
    rejectUnknownKeys(override, PAGE_OVERRIDE_FIELDS, `field of pages.${rule}`);

    const page: PageOverride = {};

    for (const band of PAGE_OVERRIDE_FIELDS) {
      const value = override[band];

      if (value == null) continue;
      if (value === false) {
        page[band] = false;
        continue;
      }
      const { node, css } = await resolveBand(value);

      page[band] = node;
      resolved.nodes.push(node);
      resolved.css.push(...css);
    }
    resolved.pages[rule] = page;
  }
  return resolved;
}

/** What the wasm side reads a document as: HTML as written, a tree as JSON when it can, or JSX rendered. */
async function resolveNode(
  input: NodeInput,
): Promise<{ node: Node | string | JsonSource; css: string[] }> {
  const { node, css } = await resolveBand(input);

  return { node: typeof node === "string" ? node : toNativeSource(node), css };
}

/** What the wasm side reads a band as: HTML as written, a tree, or JSX rendered. */
async function resolveBand(input: NodeInput): Promise<{ node: Node | string; css: string[] }> {
  if (typeof input === "string" || isNode(input)) {
    return { node: input, css: [] };
  }

  return fromJsx(input as ReactNode);
}

/**
 * A document's main tree and bands as the wasm side reads them, every tree `needs` reads, and the
 * layout options that carry the bands and the CSS they bring.
 */
async function resolveDocument(
  node: NodeInput,
  {
    header,
    footer,
    pages,
    css,
  }: Pick<RenderOptions & PagedOptions, "header" | "footer" | "pages" | "css">,
) {
  const [main, headerResult, footerResult, pagesResult] = await Promise.all([
    resolveNode(node),
    header == null || header === false ? undefined : resolveBand(header),
    footer == null || footer === false ? undefined : resolveBand(footer),
    pages == null ? undefined : resolveRules(pages),
  ]);
  const bands = [headerResult?.node, footerResult?.node, ...(pagesResult?.nodes ?? [])].filter(
    (band) => band !== undefined,
  );
  const sheets = [
    ...ownCss(css),
    ...main.css,
    ...(headerResult?.css ?? []),
    ...(footerResult?.css ?? []),
    ...(pagesResult?.css ?? []),
  ];

  return {
    node: main.node,
    trees: [main.node, ...bands],
    layout: {
      header: headerResult?.node,
      footer: footerResult?.node,
      pages: pagesResult?.pages,
      css: sheets.length > 0 ? sheets : undefined,
    },
  };
}

/** A PDF renderer holding registered fonts. Reuse one instance across renders. */
export class PdfRenderer {
  private inner: PdfRendererInternal;
  private fonts: FontRegistry<RegisteredFamily>;

  constructor(options?: FontCacheOptions) {
    this.inner = new PdfRendererInternal(options);
    this.fonts = new FontRegistry(this.inner);
  }

  /**
   * Reads what a document and its bands need under `options`, then loads the font subsets their
   * text uses and fetches the images they draw, so {@link render} and {@link measure} only lay the
   * document out and write it.
   */
  async prepare(node: NodeInput, options: PrepareOptions = {}): Promise<PreparedSource> {
    const { fonts, images, signal, fontFamilies, header, footer, pages, css, ...rest } = options;
    const document = await resolveDocument(node, { header, footer, pages, css });
    const layout = { ...rest, ...document.layout };

    return prepareSource(
      this.fonts,
      document.node,
      layout,
      { fonts, images, signal, fontFamilies },
      (svgs) => this.needs(document.trees, layout, svgs),
    );
  }

  /** Renders a node tree, JSX, an HTML string, or a prepared document to PDF bytes. */
  render(document: PreparedSource, options?: PreparedRenderOptions): Promise<Uint8Array>;
  render(node: NodeInput, options?: RenderOptions): Promise<Uint8Array>;
  async render(node: PreparedSource | NodeInput, options: RenderOptions = {}): Promise<Uint8Array> {
    return renderPrepared(
      this.fonts,
      node instanceof Prepared ? node : await this.read(node, options),
      node instanceof Prepared ? options : { signal: options.signal },
      (native, opts) => this.inner.render(native, opts),
    );
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
  measure(document: PreparedSource): Promise<MeasuredSize>;
  measure(node: NodeInput, options?: MeasureOptions): Promise<MeasuredSize>;
  async measure(
    node: PreparedSource | NodeInput,
    options: MeasureOptions = {},
  ): Promise<MeasuredSize> {
    if (node instanceof Prepared) {
      return renderPrepared(this.fonts, node, {}, (native, opts) =>
        this.inner.measure(native, opts),
      );
    }

    const { fonts, images, css, fontFamilies, ...rest } = options;
    const main = await resolveNode(node);
    const sheets = [...ownCss(css), ...main.css];
    const layout = { ...rest, css: sheets.length > 0 ? sheets : undefined };

    // The tree measured may itself be a band, so its counters are always in play.
    const prepared = await readSource(main.node, layout, { fonts, images, fontFamilies }, (svgs) =>
      this.needs([main.node], layout, svgs),
    );

    return renderPrepared(this.fonts, prepared, {}, (native, opts) =>
      this.inner.measure(native, opts),
    );
  }

  /**
   * The images `trees` look up by URL and the characters they shape after the cascade, page
   * counters their classes hook included.
   */
  private needs(
    trees: (Node | string | JsonSource)[],
    options: Parameters<PdfRendererInternal["needs"]>[1],
    svgs?: ImageSource[],
  ): SourceNeeds {
    return joinNeeds(trees.map((tree) => this.inner.needs(tree, { ...options, images: svgs })));
  }

  /** A document and its bands read under `options` without fetching, for {@link render}. */
  private async read(node: NodeInput, options: RenderOptions): Promise<PreparedSource> {
    const { fonts, images, header, footer, pages, css, fontFamilies, signal, ...rest } = options;
    const document = await resolveDocument(node, { header, footer, pages, css });
    const layout = { ...rest, ...document.layout };

    return readSource(document.node, layout, { fonts, images, signal, fontFamilies }, (svgs) =>
      this.needs(document.trees, layout, svgs),
    );
  }

  /** Registers a font ahead of time, deduped against earlier registrations. */
  registerFont(font: FontLoader): Promise<RegisteredFamily[]> {
    return this.fonts.register(font);
  }

  /**
   * Removes every face registered under the family `name`, from any `registerFont` call and each
   * `unicodeRange` subset alike; returns whether it had any. Takes a family `registerFont` returned
   * as well. A later render that lists one of its fonts registers it again.
   */
  unregisterFont(family: RegisteredFamilyLike): boolean {
    return this.inner.unregisterFont(family.name);
  }

  /** Removes every registered font, leaving the bundled fallback font. */
  clearFonts(): void {
    this.inner.clearFonts();
  }

  /** Releases the underlying wasm renderer's memory. */
  free(): void {
    this.inner.free();
  }
}

let shared: PdfRenderer | undefined;

/** Prepares with a lazily created shared {@link PdfRenderer}. */
export function prepare(node: NodeInput, options?: PrepareOptions): Promise<PreparedSource> {
  shared ??= new PdfRenderer();
  return shared.prepare(node, options);
}

/** Renders with a lazily created shared {@link PdfRenderer}. */
export function render(
  document: PreparedSource,
  options?: PreparedRenderOptions,
): Promise<Uint8Array>;
export function render(node: NodeInput, options?: RenderOptions): Promise<Uint8Array>;
export function render(
  node: PreparedSource | NodeInput,
  options?: RenderOptions,
): Promise<Uint8Array> {
  shared ??= new PdfRenderer();
  return node instanceof Prepared ? shared.render(node, options) : shared.render(node, options);
}

/** Measures with a lazily created shared {@link PdfRenderer}. */
export function measure(document: PreparedSource): Promise<MeasuredSize>;
export function measure(node: NodeInput, options?: MeasureOptions): Promise<MeasuredSize>;
export function measure(
  node: PreparedSource | NodeInput,
  options?: MeasureOptions,
): Promise<MeasuredSize> {
  shared ??= new PdfRenderer();
  return node instanceof Prepared ? shared.measure(node) : shared.measure(node, options);
}
