import type { CssInput, Node } from "@takumi-rs/helpers";
import { type DocumentInput, resolveDocument } from "./document";
import { type JsonSource, toNativeSource } from "./node-source";
import {
  type NeedsRequest,
  type Prepared,
  type PrepareImagesInput,
  type prepareSource,
  type SourceExtras,
  type SourceReader,
  splitExtras,
} from "./prepared";
import type {
  FontLoader,
  FontRegistry,
  FontScope,
  ImageSource,
  ImagesInput,
  RegisteredFamilyLike,
} from "./renderer";
import { sheetsOf } from "./wasm-renderer";

/** A band on a page: a document input, or `false` for none. */
export type BandInput = DocumentInput | false;

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

export type PdfRenderOptions = (PagedOptions | ViewportOptions) &
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

/** Options for preparing a PDF document: everything {@link PdfRenderOptions} lays out with. */
export type PdfPrepareOptions = DistributiveOmit<PdfRenderOptions, OutputOnly | "images"> & {
  /** Images to use as they are, and how to fetch the rest the document draws. */
  images?: PrepareImagesInput;
  signal?: AbortSignal;
};

/** Options for rendering a {@link PreparedPdf}: how its pages are written to the file. */
export type PdfPreparedRenderOptions = ConformanceOptions &
  Pick<
    PagedOptions & PdfRenderOptions,
    "metadata" | "outline" | "backgroundColor" | "uncoveredText"
  > & {
    signal?: AbortSignal;
  };

/** A PDF document a renderer prepared, with its fonts and images loaded. */
export type PreparedPdf = Prepared<Node | string | JsonSource, PdfLayout>;

/** The binding options a document lays out with, its bands resolved to what the binding reads. */
type PdfLayout = DistributiveOmit<
  PdfPrepareOptions,
  keyof SourceExtras | "css" | "header" | "footer" | "pages"
> & {
  header?: Node | string;
  footer?: Node | string;
  pages?: ResolvedPageRules;
  css?: CssInput[];
};

const PAGE_RULES = ["first", "last", "odd", "even"] as const;
const PAGE_OVERRIDE_FIELDS = ["header", "footer"] as const;

/** A band as the binding reads it: a node tree, HTML, or `false` for none. */
type ResolvedBand = Node | string | false;

/** What some pages draw instead of the document's own bands, resolved for the binding. */
type ResolvedPageOverride = { header?: ResolvedBand; footer?: ResolvedBand };

/** Page rules with every band resolved for the binding. */
type ResolvedPageRules = Partial<Record<(typeof PAGE_RULES)[number], ResolvedPageOverride>>;

/** Page rules with every document input resolved to a node tree. */
type ResolvedRules = { pages: ResolvedPageRules; css: string[] };

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
  const resolved: ResolvedRules = { pages: {}, css: [] };

  rejectNonObject(input, "pages");
  rejectUnknownKeys(input, PAGE_RULES, "page rule");
  for (const rule of PAGE_RULES) {
    const override = input[rule];

    if (override == null) {
      continue;
    }
    rejectNonObject(override, `pages.${rule}`);
    rejectUnknownKeys(override, PAGE_OVERRIDE_FIELDS, `field of pages.${rule}`);

    const page: ResolvedPageOverride = {};

    for (const band of PAGE_OVERRIDE_FIELDS) {
      const value = override[band];

      if (value == null) {
        continue;
      }
      if (value === false) {
        page[band] = false;
        continue;
      }
      const { node, css } = await resolveDocument(value);

      page[band] = node;
      resolved.css.push(...css);
    }
    resolved.pages[rule] = page;
  }
  return resolved;
}

/**
 * What a binding reads a document as: HTML as written, a tree as JSON when it can, or JSX rendered.
 *
 * @internal
 */
export async function resolveNode(
  input: DocumentInput,
): Promise<{ node: Node | string | JsonSource; css: string[] }> {
  const { node, css } = await resolveDocument(input);

  return { node: toNativeSource(node), css };
}

/** A document's main tree as the binding reads it, and the layout options that carry its bands and the CSS they bring. */
async function resolveLayout(
  node: DocumentInput,
  {
    header,
    footer,
    pages,
    css,
  }: Pick<PdfRenderOptions & PagedOptions, "header" | "footer" | "pages" | "css">,
) {
  const [main, headerResult, footerResult, pagesResult] = await Promise.all([
    resolveNode(node),
    header == null || header === false ? undefined : resolveDocument(header),
    footer == null || footer === false ? undefined : resolveDocument(footer),
    pages == null ? undefined : resolveRules(pages),
  ]);

  return {
    node: main.node,
    layout: {
      header: headerResult?.node,
      footer: footerResult?.node,
      pages: pagesResult?.pages,
      css: sheetsOf(css, [
        ...main.css,
        ...(headerResult?.css ?? []),
        ...(footerResult?.css ?? []),
        ...(pagesResult?.css ?? []),
      ]),
    },
  };
}
/**
 * Every variant of {@link PdfRenderOptions} in the one shape a binding's PDF passes read, its bands resolved.
 *
 * @internal
 */
export type PdfBindingOptions = {
  viewport?: ViewportInput;
  size?: PageSize;
  landscape?: boolean;
  margin?: PageMargin;
  header?: ResolvedBand;
  footer?: ResolvedBand;
  pages?: ResolvedPageRules;
  pageRanges?: PageRange[];
  images?: ImageSource[];
  css?: CssInput[];
  fontFamilies?: string[];
  fontScope?: FontScope;
  lang?: string;
  metadata?: PdfMetadata;
  outline?: boolean;
  backgroundColor?: string;
  uncoveredText?: UncoveredText;
  pdfa?: PdfRenderOptions["pdfa"];
  tagged?: PdfRenderOptions["tagged"];
  attachments?: Attachment[];
};

/**
 * Resolves a PDF document's tree and bands, then collects what `read`, a binding's needs pass, reports.
 *
 * @internal
 */
export async function collectPdf<TFamily extends RegisteredFamilyLike>(
  registry: FontRegistry<TFamily>,
  node: DocumentInput,
  options: PdfPrepareOptions | PdfRenderOptions,
  collect: typeof prepareSource,
  read: (
    source: Node | string | JsonSource,
    layout: PdfLayout,
    request: NeedsRequest,
    signal: AbortSignal | undefined,
  ) => ReturnType<SourceReader>,
): Promise<PreparedPdf> {
  const {
    extras,
    layout: { header, footer, pages, css, ...rest },
  } = splitExtras(options);

  extras.signal?.throwIfAborted();

  const document = await resolveLayout(node, { header, footer, pages, css });
  const layout = { ...rest, ...document.layout };

  return collect(registry, document.node, layout, extras, (request) =>
    read(document.node, layout, request, extras.signal),
  );
}
