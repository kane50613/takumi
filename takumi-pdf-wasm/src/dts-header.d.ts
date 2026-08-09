import type { Node } from "@takumi-rs/helpers";

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

/** Cache policy for a decoded image. Defaults to `"auto"`. */
export type ImageCacheMode = "auto" | "none";

export type FontDetails = {
  name?: string;
  data: ByteBuf;
  weight?: number;
  style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
  /**
   * Logical family this font is a coverage subset of. Subsets sharing a
   * `subsetOf` are kept as distinct families and `font-family: {subsetOf}`
   * expands to all of them, so each script routes to the subset that covers it.
   */
  subsetOf?: string;
  /**
   * Where this subset sits in its group's fallback order; lowest is tried first, and equal
   * ranks order by family name. A subset's `cmap` reaches past the range it was cut for, so
   * the rank is what settles which subset serves a codepoint several of them encode.
   */
  subsetRank?: number;
  /** CSS generic family keyword this font resolves for. */
  generic?: string;
};

export type Font = FontDetails | ByteBuf;

export type RegisteredFace = {
  weight: number;
  style: string;
  width: number;
  index: number;
};

export type RegisteredFamily = {
  name: string;
  faces: RegisteredFace[];
};

export type ImageSource = {
  src: string;
  data: ByteBuf;
  /** Cache policy for the decoded image. Defaults to `"auto"`. */
  cache?: ImageCacheMode;
};

/** Explicit page or viewport dimensions in CSS px (96 dpi). */
export type Dimensions = { width: number; height: number };

/** Viewport for single-page output. A missing height sizes the page to the content. */
export type ViewportInput = { width: number; height?: number };

/** A page size: a preset name (case-insensitive) or explicit dimensions. */
export type PageSize = "a4" | "letter" | Dimensions;

/** A page margin in CSS px: one number for all sides, or per-side values (missing sides are zero). */
export type PageMargin = number | { top?: number; right?: number; bottom?: number; left?: number };

export type PdfMetadata = {
  title?: string;
  description?: string;
  authors?: string[];
  keywords?: string[];
  creator?: string;
  /** UTC creation date, `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`. */
  creationDate?: string;
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

/** PDF/A conformance level. Validation failures reject the render. */
export type Pdfa = "2a" | "2b" | "2u" | "3a" | "3b" | "3u" | "4" | "4f";

/** Structure-tree emission: off, on (default), or validated against an accessibility standard. */
export type Tagged = boolean | "ua1" | "ua2";

export type PdfRenderOptions = {
  /**
   * Fixed viewport for single-page output. Mutually exclusive with the paged
   * fields (`size`, `landscape`, `margin`, `header`, `footer`).
   */
  viewport?: ViewportInput;
  /** Page size for paged output. Defaults to A4. */
  size?: PageSize;
  /** Swaps the page's width and height, including explicit sizes. */
  landscape?: boolean;
  /** Page margin in CSS px. Defaults to a uniform 48 (half an inch). */
  margin?: PageMargin;
  /**
   * Band repeated at the top of every page. Nodes classed `pageNumber` /
   * `totalPages` receive the counters.
   */
  header?: Node;
  /** Band repeated at the bottom of every page; same class hooks as `header`. */
  footer?: Node;
  /** Pre-fetched images keyed by URL. */
  images?: ImageSource[];
  /** CSS stylesheets to apply before layout. */
  stylesheets?: string[];
  /** Per-render font stack: ordered family names used as the fallback chain. */
  fontFamilies?: string[];
  /** Default BCP-47 language tag applied to the root. */
  lang?: string;
  /** Document metadata written to the PDF's info dictionary. */
  metadata?: PdfMetadata;
  /** Generates a PDF outline (bookmarks) from `h1`–`h6` headings. */
  outline?: boolean;
  /** PDF/A conformance level. */
  pdfa?: Pdfa;
  /** Structure-tree emission: `false`, `true` (default), `"ua1"` or `"ua2"`. */
  tagged?: Tagged;
  /** Files attached to the document. */
  attachments?: Attachment[];
};

/** Options for `measure`: page geometry (or a viewport) plus layout resources. */
export type MeasureOptions = (
  | { size?: PageSize; landscape?: boolean; viewport?: never }
  | { viewport: ViewportInput; size?: never; landscape?: never }
) &
  Pick<PdfRenderOptions, "images" | "stylesheets" | "fontFamilies" | "lang">;

/** A node tree's laid-out size in CSS px. */
export type MeasuredSize = { width: number; height: number };
