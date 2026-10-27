import type { GoogleFontShapeFamilies, GoogleFontShapes } from "./google-fonts-catalog";
import type { HtmlSource, Node } from "./types";
import type { FetchOptions } from "./fetch";
import { fetchBytes } from "./fetch/bytes";
import { FontStylesheetCache } from "./fonts/cache";
import { unicodeRangeOf } from "./unicode-range";

type FontStyle = "normal" | "italic";

/** CSS generic family keywords a font can claim. */
export type GenericFontFamily =
  | "serif"
  | "sans-serif"
  | "monospace"
  | "cursive"
  | "fantasy"
  | "system-ui"
  | "ui-serif"
  | "ui-sans-serif"
  | "ui-monospace"
  | "ui-rounded"
  | "emoji"
  | "math"
  | "fangsong";
type WeightRange = `${number}..${number}`;
type AxisValue = number | WeightRange;

// One branch per distinct weight/style/axis shape, not per family: same-shape families are
// indistinguishable here, so grouping keeps this union ~13x smaller for the checker.
type KnownGoogleFontFamily = {
  [S in keyof GoogleFontShapes]: {
    name: GoogleFontShapeFamilies[S];
    /** `400`, `[400, 700]`, or a range like `"100..900"` for the variable font. @default 400 */
    weight?: GoogleFontShapes[S]["weight"] | GoogleFontShapes[S]["weight"][] | WeightRange;
    /** `"normal"`, `"italic"`, or both. @default "normal" */
    style?: GoogleFontShapes[S]["style"] | GoogleFontShapes[S]["style"][];
    /** Variable axes to vary, each at a value or `"min..max"` range, e.g. `{ opsz: "14..32" }`. */
    axes?: [GoogleFontShapes[S]["axis"]] extends [never]
      ? never
      : Partial<Record<GoogleFontShapes[S]["axis"], AxisValue>>;
    /** CSS generic family the loaded subsets claim, e.g. `"monospace"`. */
    generic?: GenericFontFamily;
  };
}[keyof GoogleFontShapes];

type GoogleFontName = GoogleFontShapeFamilies[keyof GoogleFontShapeFamilies];

/**
 * One family to load. A bare string loads weight 400, normal style. The object form autocompletes
 * each known family's weight, style, and variable axes; the last branch keeps any string name
 * (e.g. one built at runtime) usable with a weight and style.
 */
export type GoogleFontFamily =
  | GoogleFontName
  | (string & {})
  | KnownGoogleFontFamily
  | {
      name: GoogleFontName | (string & {});
      weight?: number | number[] | WeightRange;
      style?: FontStyle | FontStyle[];
      axes?: Record<string, AxisValue>;
      /** CSS generic family the loaded subsets claim, e.g. `"monospace"`. */
      generic?: GenericFontFamily;
    };

export type GoogleFontsOptions = FetchOptions & {
  /** The families to load, each as a name or a name plus its weight/style axis. */
  families: GoogleFontFamily[];
  /** `font-display` strategy passed through to the CSS request. */
  display?: "auto" | "block" | "swap" | "fallback" | "optional";
  /** CSS promise cache; defaults to a bounded cache per fetch implementation. Signals and URL policies bypass caching. */
  cache?: Pick<Map<string, Promise<string>>, "get" | "set" | "delete">;
  /**
   * css2 base URL. Defaults to Google Fonts; point it at an API-compatible mirror like
   * `https://fonts.bunny.net/css2`.
   */
  baseUrl?: string;
};

const GOOGLE_FONTS_CSS = "https://fonts.googleapis.com/css2";

// Builds a css2 `family=` value, e.g. `Inter:ital,opsz,wght@0,14..32,400`. Takes plain values
// rather than a GoogleFontFamily so it never relates that ~2000-member union to a parameter, which
// overflows the type checker (TS2590).
function familyValue(
  family: string,
  weights: string[],
  styles: string[],
  customAxes: [string, string][],
) {
  const axisValues = new Map(customAxes);
  const hasItalic = styles.includes("italic");
  const italics = hasItalic ? (styles.includes("normal") ? [0, 1] : [1]) : [undefined];

  const tags = [...(hasItalic ? ["ital"] : []), ...axisValues.keys(), "wght"].sort();

  const tuples = italics
    .flatMap((ital) =>
      weights.map((w) =>
        tags
          .map((tag) => {
            if (tag === "ital") {
              return String(ital);
            }
            return tag === "wght" ? w : (axisValues.get(tag) ?? "");
          })
          .join(","),
      ),
    )
    .sort();

  return `${family}:${tags.join(",")}@${tuples.join(";")}`;
}

function buildUrl(options: GoogleFontsOptions) {
  const url = new URL(options.baseUrl ?? GOOGLE_FONTS_CSS);
  for (const family of options.families) {
    if (typeof family === "string") {
      url.searchParams.append("family", familyValue(family, ["400"], ["normal"], []));
      continue;
    }

    const weight = family.weight ?? 400;
    const weights = (Array.isArray(weight) ? [...weight].sort((a, b) => a - b) : [weight]).map(
      String,
    );
    const style = family.style ?? "normal";
    const styles = (Array.isArray(style) ? style : [style]).map(String);
    const customAxes = Object.entries(family.axes ?? {})
      .filter(([tag]) => tag !== "ital" && tag !== "wght")
      .map(([tag, value]): [string, string] => [tag, String(value)]);

    url.searchParams.append("family", familyValue(family.name, weights, styles, customAxes));
  }
  if (options.display) {
    url.searchParams.set("display", options.display);
  }
  return url.toString();
}

const fontFaceBlockPattern = /(?:\/\*\s*([^*]+?)\s*\*\/\s*)?@font-face\s*\{([^}]*)\}/g;

type SubsetFace = {
  /** Logical family from the block's `font-family`; groups subsets across one request. */
  family: string;
  subset: string;
  url: string;
  weight?: number;
  style?: string;
  ranges: [number, number][];
};

/** A loaded font subset, ready to hand to a renderer's `fonts`. */
export type FontSubset = {
  /** The family authors reference in `font-family`, shared by every subset of it. */
  name: string;
  /** Stable dedup key (`"{name} {subset}:{weight}:{style}:{range}"`), independent of the rotating woff2 URL. */
  key: string;
  weight?: number;
  style?: string;
  /** Inclusive codepoint ranges from the face's `unicode-range`; empty covers everything. */
  unicodeRange: [number, number][];
  /** CSS generic family the subset claims, from the family's `generic` option. */
  generic?: GenericFontFamily;
  /** Fetches the subset bytes. The renderer skips files it has already loaded. */
  data: () => Promise<ArrayBuffer>;
};

function toSubset(
  face: SubsetFace,
  options: FetchOptions,
  generic?: GenericFontFamily,
): FontSubset {
  const range = face.ranges.map(([lo, hi]) => `${lo}-${hi}`).join(",");

  return {
    name: face.family,
    // Stable coverage identity, not the woff2 URL Google may rotate, so the renderer dedups
    // across calls yet never merges two subsets that cover different ranges.
    key: `${face.family} ${face.subset}:${face.weight ?? ""}:${face.style ?? ""}:${range}:${generic ?? ""}`,
    weight: face.weight,
    style: face.style,
    unicodeRange: face.ranges,
    generic,
    data: () => fetchBytes(face.url, options),
  };
}

/** Parse `U+0460-052F, U+20B4, U+30??` into inclusive `[lo, hi]` codepoint ranges. */
function parseUnicodeRange(value: string): [number, number][] {
  const ranges: [number, number][] = [];

  for (const raw of value.split(",")) {
    const token = raw.trim().replace(/^U\+/i, "");
    if (!token) {
      continue;
    }

    if (token.includes("-")) {
      const [lo, hi] = token.split("-");
      if (lo && hi) {
        ranges.push([parseInt(lo, 16), parseInt(hi, 16)]);
      }
    } else if (token.includes("?")) {
      ranges.push([
        parseInt(token.replace(/\?/g, "0"), 16),
        parseInt(token.replace(/\?/g, "F"), 16),
      ]);
    } else {
      const cp = parseInt(token, 16);
      ranges.push([cp, cp]);
    }
  }

  return ranges;
}

/** Parse each `@font-face` block, keeping its subset label and `unicode-range`. */
function parseSubsetFaces(css: string): SubsetFace[] {
  const faces: SubsetFace[] = [];

  let index = 0;
  for (const match of css.matchAll(fontFaceBlockPattern)) {
    const body = match[2];
    if (!body) {
      continue;
    }

    const url = body
      .match(/src:\s*url\(([^)]+)\)/)?.[1]
      ?.replace(/['"]/g, "")
      .trim();
    const family = body.match(/font-family:\s*['"]?([^'";]+)['"]?/)?.[1]?.trim();
    if (!url || !family) {
      continue;
    }

    const range = body.match(/unicode-range:\s*([^;]+)/)?.[1];
    const weight = body.match(/font-weight:\s*(\d+)(?:\s+(\d+))?/);

    faces.push({
      family,
      subset: match[1]?.trim() || `subset-${index}`,
      url,
      weight: weight && !weight[2] ? Number(weight[1]) : undefined,
      style: body.match(/font-style:\s*([a-z]+)/i)?.[1],
      // No `unicode-range` (e.g. a full static font) means the face covers everything.
      ranges: range ? parseUnicodeRange(range) : [],
    });
    index += 1;
  }

  return faces;
}

/**
 * A variable font reuses one woff2 across weights: the same `src` url repeats per subset, one
 * `@font-face` per weight. Collapse each shared url to a single weightless face so the renderer
 * drives the `wght` axis. Registering the file once per weight pins every weight to the file's
 * default, so `font-weight: 700` renders regular. Static fonts give a distinct url per weight and
 * pass through untouched.
 */
function mergeVariableFaces(faces: SubsetFace[]): SubsetFace[] {
  const weightsByUrl = new Map<string, Set<number | undefined>>();
  for (const face of faces) {
    const weights = weightsByUrl.get(face.url) ?? new Set();
    weights.add(face.weight);
    weightsByUrl.set(face.url, weights);
  }

  const seen = new Set<string>();
  const merged: SubsetFace[] = [];
  for (const face of faces) {
    if (seen.has(face.url)) {
      continue;
    }

    seen.add(face.url);
    const sharedAcrossWeights = (weightsByUrl.get(face.url)?.size ?? 0) > 1;
    merged.push(sharedAcrossWeights ? { ...face, weight: undefined } : face);
  }

  return merged;
}

/**
 * A codepoint source: content trees, plus loose strings for text a backend injects that no
 * node carries, such as the digits a PDF page counter fills in.
 */
export type CodepointSource = string | Node | HtmlSource | (string | Node | HtmlSource)[];

/**
 * Every character the predefined list marker styles can generate. Markers are
 * generated content no node carries, so subsetting counts these as always
 * used. Mirrors `MARKER_CHARACTERS` in takumi-core, whose coverage test is the
 * source of truth. `list-style-type: "…"` strings are not covered.
 */
export const LIST_MARKER_CHARACTERS =
  "•◦■▪▸◂▾ 0123456789.-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/** Collect every codepoint the content will render. */
export function collectCodepoints(source: CodepointSource): Set<number> {
  const codepoints = new Set<number>();

  const add = (text: string) => {
    for (const ch of text) {
      codepoints.add(ch.codePointAt(0) as number);
    }
  };
  const walk = (node: string | Node | HtmlSource) => {
    if (typeof node === "string") {
      add(node);
    } else if ("html" in node) {
      add(node.html);
    } else if (node.type === "text") {
      add(node.text);
    } else if (node.type === "container") {
      node.children?.forEach(walk);
    }
  };

  if (Array.isArray(source)) {
    source.forEach(walk);
  } else {
    walk(source);
  }

  return codepoints;
}

/** Whether any of the ascending `codepoints` falls in `ranges`; no ranges covers everything. */
function rangesCover(ranges: [number, number][], codepoints: Uint32Array): boolean {
  return (
    ranges.length === 0 ||
    ranges.some(([lo, hi]) => {
      const first = codepoints[lowerBound(codepoints, lo)];

      return first !== undefined && first <= hi;
    })
  );
}

/** The index of the first of the ascending `values` at or above `target`. */
function lowerBound(values: Uint32Array, target: number): number {
  let low = 0;
  let high = values.length;

  while (low < high) {
    const middle = (low + high) >>> 1;

    if ((values[middle] ?? target) < target) {
      low = middle + 1;
    } else {
      high = middle;
    }
  }

  return low;
}

/**
 * Keep the subsets `source` renders, dropping the rest. Range-less fallbacks (a full font or a
 * `text=` subset) always stay. Runs without network, so a renderer can apply it to the final
 * node tree. Works on any descriptor carrying `unicodeRange`; entries without it are kept.
 */
export function subsetFonts<T>({ fonts, source }: { fonts: T[]; source: CodepointSource }): T[] {
  const codepoints = Uint32Array.from(collectCodepoints(source)).sort();

  return fonts.filter((font) => rangesCover(unicodeRangeOf(font), codepoints));
}

/**
 * Turn a font URL into a loader: fetches the bytes on demand, keyed by the URL so repeated renders
 * dedupe. Family name, weight, and style come from the font file. Lets `fonts` take a bare URL
 * string, e.g. `fonts: ["https://example.com/Inter.woff2"]`.
 */
export function fontFromUrl(
  url: string,
  options: FetchOptions = {},
): { key: string; data: () => Promise<ArrayBuffer> } {
  return {
    key: url,
    data: () => fetchBytes(url, options),
  };
}

/**
 * Load Google Font families in one css2 request, returning every coverage subset. Each registers
 * under its family name with its `unicode-range`, so a codepoint routes to the subset that
 * declares it, as in a browser.
 * Subsets download lazily. Hand the result to `render`, which registers only the subsets the
 * content uses; or trim them first with {@link subsetFonts}.
 *
 * @example
 * const fonts = await googleFonts(["Inter", "Noto Sans JP"]);
 * await render(element, { width, height, fonts });
 */
export async function googleFonts(
  options: GoogleFontsOptions | GoogleFontFamily[],
): Promise<FontSubset[]> {
  if (Array.isArray(options)) {
    options = { families: options };
  }

  if (options.families.length === 0) {
    return [];
  }

  const css = await FontStylesheetCache.load(buildUrl(options), options);

  // Google's css2 response orders `@font-face` blocks by its own logic, not by the `family=`
  // query order — the caller's declared priority survives only if we restore it here. A render
  // with no explicit `fontFamilies` falls back to registration order, so this is what lets
  // `families: ["Noto Sans TC", "Noto Sans JP"]` actually prefer the Traditional Chinese face
  // for a codepoint both cover.
  const familyOrder = new Map(
    options.families.map((family, index) => [
      typeof family === "string" ? family : family.name,
      index,
    ]),
  );
  const generics = new Map(
    options.families.flatMap((family) =>
      typeof family === "string" || !family.generic ? [] : [[family.name, family.generic] as const],
    ),
  );

  return mergeVariableFaces(parseSubsetFaces(css))
    .sort((a, b) => (familyOrder.get(a.family) ?? 0) - (familyOrder.get(b.family) ?? 0))
    .map((face) => toSubset(face, options, generics.get(face.family)));
}
