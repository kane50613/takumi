import type { GoogleFontShapeFamilies, GoogleFontShapes } from "./google-fonts-catalog";
import type { Node } from "./types";
import { defaultMaxFetchBytes, fetchOk, type FetchOptions, readBodyLimited } from "./utils";

const chromeUserAgent =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

type FontStyle = "normal" | "italic";
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
    };

export type GoogleFontsOptions = FetchOptions & {
  /** The families to load, each as a name or a name plus its weight/style axis. */
  families: GoogleFontFamily[];
  /** `font-display` strategy passed through to the CSS request. */
  display?: "auto" | "block" | "swap" | "fallback" | "optional";
  /**
   * Cache for the Google Fonts CSS, keyed by request URL. Reuse one across renders (e.g. a
   * playground re-rendering on each edit) so the metadata is fetched and parsed once.
   */
  cache?: Pick<Map<string, string>, "has" | "get" | "set">;
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

const maxCssBytes = 2 * 1024 * 1024;

function fetchCss(url: string, options: FetchOptions) {
  return fetchOk(url, {
    ...options,
    init: { headers: { "User-Agent": chromeUserAgent } },
  })
    .then((r) => readBodyLimited(r, options.maxBytes ?? maxCssBytes))
    .then((buffer) => new TextDecoder().decode(buffer));
}

async function fetchCssCached(url: string, options: GoogleFontsOptions) {
  const cached = options.cache?.get(url);
  if (cached !== undefined) {
    return cached;
  }

  const css = await fetchCss(url, options);
  options.cache?.set(url, css);
  return css;
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
  /** Unique internal family name (`"{family} {subset}"`); never written in CSS. */
  name: string;
  /** Logical family authors reference in `font-family`; expands to every loaded subset. */
  subsetOf: string;
  /** Stable dedup key (`"{name}:{weight}:{style}:{range}"`), independent of the rotating woff2 URL. */
  key: string;
  weight?: number;
  style?: string;
  /** Inclusive codepoint ranges this subset covers; empty covers everything. */
  ranges: [number, number][];
  /** Fetches the subset bytes. The renderer skips files it has already loaded. */
  data: () => Promise<ArrayBuffer>;
};

function toSubset(face: SubsetFace, options: FetchOptions): FontSubset {
  const name = `${face.family} ${face.subset}`;
  const range = face.ranges.map(([lo, hi]) => `${lo}-${hi}`).join(",");

  return {
    name,
    subsetOf: face.family,
    // Stable coverage identity, not the woff2 URL Google may rotate, so the renderer dedups
    // across calls yet never merges two subsets that cover different ranges.
    key: `${name}:${face.weight ?? ""}:${face.style ?? ""}:${range}`,
    weight: face.weight,
    style: face.style,
    ranges: face.ranges,
    data: () =>
      fetchOk(face.url, options).then((r) =>
        readBodyLimited(r, options.maxBytes ?? defaultMaxFetchBytes),
      ),
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

/** Collect every codepoint the content will render. */
export function collectCodepoints(source: string | Node | Node[]): Set<number> {
  const codepoints = new Set<number>();

  const add = (text: string) => {
    for (const ch of text) {
      codepoints.add(ch.codePointAt(0) as number);
    }
  };
  const walk = (node: Node) => {
    if (node.type === "text") {
      add(node.text);
    } else if (node.type === "container") {
      node.children?.forEach(walk);
    }
  };

  if (typeof source === "string") {
    add(source);
  } else if (Array.isArray(source)) {
    source.forEach(walk);
  } else {
    walk(source);
  }

  return codepoints;
}

function rangesCover(ranges: [number, number][], codepoints: Set<number>): boolean {
  if (ranges.length === 0) {
    return true;
  }

  for (const cp of codepoints) {
    for (const [lo, hi] of ranges) {
      if (cp >= lo && cp <= hi) {
        return true;
      }
    }
  }

  return false;
}

/**
 * Keep the subsets `source` renders, dropping the rest. Range-less fallbacks (a full font or a
 * `text=` subset) always stay. Runs without network, so a renderer can apply it to the final
 * node tree. Works on any descriptor carrying `ranges`; entries without it are kept.
 */
export function subsetFonts<T>({
  fonts,
  source,
}: {
  fonts: T[];
  source: string | Node | Node[];
}): T[] {
  const codepoints = collectCodepoints(source);

  return fonts.filter((font) =>
    rangesCover((font as { ranges?: [number, number][] }).ranges ?? [], codepoints),
  );
}

/**
 * Turn a font URL into a loader: fetches the bytes on demand, keyed by the URL so repeated renders
 * dedupe. Family name, weight, and style come from the font file. Lets `fonts` take a bare URL
 * string, e.g. `fonts: ["https://example.com/Inter.woff2"]`.
 */
export function fontFromUrl(url: string, options: FetchOptions = {}) {
  return {
    key: url,
    data: () =>
      fetchOk(url, options).then((r) =>
        readBodyLimited(r, options.maxBytes ?? defaultMaxFetchBytes),
      ),
  };
}

/**
 * Load Google Font families in one css2 request, returning every coverage subset. Each keeps
 * its `unicode-range` and registers uniquely-named under its `subsetOf` family, which
 * `font-family: {family}` expands across, so a glyph routes to the subset that covers it.
 * Subsets download lazily. Hand the result to `render`, which registers only the subsets the
 * content uses; or trim them first with {@link subsetFonts}.
 *
 * @example
 * const fonts = await googleFonts({ families: ["Inter", "Noto Sans JP"] });
 * await render(element, { width, height, fonts });
 */
export async function googleFonts(options: GoogleFontsOptions): Promise<FontSubset[]> {
  if (options.families.length === 0) {
    return [];
  }

  const css = await fetchCssCached(buildUrl(options), options);

  return mergeVariableFaces(parseSubsetFaces(css)).map((face) => toSubset(face, options));
}
