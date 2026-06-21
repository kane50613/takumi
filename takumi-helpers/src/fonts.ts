import type { Node } from "./types";
import { fetchOk, type FetchOptions } from "./utils";

const chromeUserAgent =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

type FontStyle = "normal" | "italic";

export type GoogleFontOptions = FetchOptions & {
  /**
   * `400` for one weight, `[400, 700]` for several, or a range like `"100..900"` to load
   * the variable font. A range leaves the weight unset so CSS `font-weight` controls it.
   * @default 400
   */
  weight?: number | number[] | `${number}..${number}`;
  /** `"normal"`, `"italic"`, or both. @default "normal" */
  style?: FontStyle | FontStyle[];
  /** Limit the download to the glyphs used in this text. Recommended for OG images. */
  text?: string;
  /** `font-display` strategy passed through to the CSS request. */
  display?: "auto" | "block" | "swap" | "fallback" | "optional";
};

function buildCssUrl(
  family: string,
  { weight = 400, style = "normal", display, text }: GoogleFontOptions,
) {
  const weights = (Array.isArray(weight) ? [...weight].sort((a, b) => a - b) : [weight]).map(
    String,
  );
  const styles = Array.isArray(style) ? style : [style];

  let axis: string;
  if (styles.includes("italic")) {
    const italics = styles.includes("normal") ? [0, 1] : [1];
    axis = `ital,wght@${italics
      .flatMap((ital) => weights.map((w) => `${ital},${w}`))
      .sort()
      .join(";")}`;
  } else {
    axis = `wght@${weights.join(";")}`;
  }

  let url = `https://fonts.googleapis.com/css2?family=${encodeURIComponent(family)}:${axis}`;
  if (display) {
    url += `&display=${display}`;
  }
  if (text) {
    url += `&text=${encodeURIComponent(text)}`;
  }
  return url;
}

function fetchCss(url: string, options: FetchOptions) {
  return fetchOk(url, {
    ...options,
    init: { headers: { "User-Agent": chromeUserAgent } },
  }).then((r) => r.text());
}

const fontFaceBlockPattern = /(?:\/\*\s*([^*]+?)\s*\*\/\s*)?@font-face\s*\{([^}]*)\}/g;

type SubsetFace = {
  subset: string;
  url: string;
  weight?: number;
  style?: string;
  ranges: [number, number][];
};

/**
 * Load a Google Font as descriptors you can pass to a renderer's `fonts`. Fetches the
 * Google Fonts CSS, reads the `woff2` URLs, and returns one loader per file. Each file
 * downloads when the renderer first needs it; the renderer skips files it already loaded.
 *
 * @example
 * fonts: await googleFont("Inter", { weight: [400, 700] })
 * @example
 * fonts: await googleFont("Inter", { weight: "100..900" }) // variable
 * @example
 * fonts: await googleFont("Inter", { weight: 700, style: "italic", text: "Hello" })
 */
export async function googleFont(family: string, options: GoogleFontOptions = {}) {
  const css = await fetchCss(buildCssUrl(family, options), options);

  return parseSubsetFaces(css).map((face) => ({
    name: family,
    key: face.url,
    weight: face.weight,
    style: face.style,
    data: () => fetchOk(face.url, options).then((r) => r.arrayBuffer()),
  }));
}

/** A loaded Google Font subset, ready to hand to a renderer's `fonts`. */
export type GoogleFontSubset = {
  /** Unique internal family name (`"{family} {subset}"`); never written in CSS. */
  name: string;
  /** Logical family authors reference in `font-family`; expands to every loaded subset. */
  subsetOf: string;
  /** Subset woff2 URL, also the dedup key. */
  key: string;
  weight?: number;
  style?: string;
  /** Fetches the subset bytes; the renderer skips files it has already loaded. */
  data: () => Promise<ArrayBuffer>;
};

/** A Google family to load, plus how — same options as {@link googleFont} minus `text`. */
export type GoogleFontFamily = string | ({ family: string } & Omit<GoogleFontOptions, "text">);

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
    if (!url) {
      continue;
    }

    const range = body.match(/unicode-range:\s*([^;]+)/)?.[1];
    const weight = body.match(/font-weight:\s*(\d+)(?:\s+(\d+))?/);

    faces.push({
      subset: match[1]?.trim() || `subset-${index}`,
      url,
      weight: weight && !weight[2] ? Number(weight[1]) : undefined,
      style: body.match(/font-style:\s*([a-z]+)/i)?.[1],
      // No `unicode-range` (e.g. a `text=` request) means the face covers everything.
      ranges: range ? parseUnicodeRange(range) : [],
    });
    index += 1;
  }

  return faces;
}

/** Collect every codepoint the content will render. */
function collectCodepoints(source: string | Node | Node[]): Set<number> {
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

async function loadFamilySubsets(
  family: Exclude<GoogleFontFamily, string>,
  codepoints: Set<number>,
  options: FetchOptions,
): Promise<GoogleFontSubset[]> {
  const css = await fetchCss(buildCssUrl(family.family, family), options);

  return parseSubsetFaces(css)
    .filter((face) => rangesCover(face.ranges, codepoints))
    .map((face) => ({
      name: `${family.family} ${face.subset}`,
      subsetOf: family.family,
      key: face.url,
      weight: face.weight,
      style: face.style,
      data: () => fetchOk(face.url, options).then((r) => r.arrayBuffer()),
    }));
}

/**
 * Load only the Google Font subsets that `source` actually renders: scan its codepoints,
 * keep each family's intersecting `unicode-range` subsets, and register them under one
 * `subsetOf` family that `font-family: {family}` expands across. No font anxiety.
 *
 * @example
 * const fonts = await loadGoogleFonts(element, ["Inter", "Noto Sans JP"]);
 * await render(element, { width, height, fonts });
 */
export async function loadGoogleFonts(
  source: string | Node | Node[],
  families: GoogleFontFamily[],
  options: FetchOptions = {},
): Promise<GoogleFontSubset[]> {
  const codepoints = collectCodepoints(source);
  const specs = families.map((family) => (typeof family === "string" ? { family } : family));

  const loaded = await Promise.all(
    specs.map((spec) => loadFamilySubsets(spec, codepoints, options)),
  );

  return loaded.flat();
}
