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

const GOOGLE_FONTS_CSS = "https://fonts.googleapis.com/css2";

/** The `family=` value for one family + its weight/style axis, e.g. `Inter:wght@400`. */
function familyValue(family: string, { weight = 400, style = "normal" }: GoogleFontOptions) {
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

  return `${family}:${axis}`;
}

function buildCssUrl(family: string, options: GoogleFontOptions) {
  const url = new URL(GOOGLE_FONTS_CSS);
  url.searchParams.append("family", familyValue(family, options));
  if (options.display) {
    url.searchParams.set("display", options.display);
  }
  if (options.text) {
    url.searchParams.set("text", options.text);
  }
  return url.toString();
}

function fetchCss(url: string, options: FetchOptions) {
  return fetchOk(url, {
    ...options,
    init: { headers: { "User-Agent": chromeUserAgent } },
  }).then((r) => r.text());
}

const fontFaceBlockPattern = /(?:\/\*\s*([^*]+?)\s*\*\/\s*)?@font-face\s*\{([^}]*)\}/g;

type SubsetFace = {
  /** Logical family from the block's `font-family` (groups subsets across one request). */
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
  /** Subset woff2 URL, also the dedup key. */
  key: string;
  weight?: number;
  style?: string;
  /** Inclusive codepoint ranges this subset covers; empty means it covers everything. */
  ranges: [number, number][];
  /** Fetches the subset bytes; the renderer skips files it has already loaded. */
  data: () => Promise<ArrayBuffer>;
};

function toSubset(face: SubsetFace, options: FetchOptions): FontSubset {
  return {
    name: `${face.family} ${face.subset}`,
    subsetOf: face.family,
    key: face.url,
    weight: face.weight,
    style: face.style,
    ranges: face.ranges,
    data: () => fetchOk(face.url, options).then((r) => r.arrayBuffer()),
  };
}

/**
 * Load one Google Font family as subset descriptors you can pass to a renderer's `fonts`.
 * Each subset keeps its `unicode-range`, registers uniquely-named under its `subsetOf`
 * family (which `font-family: {family}` expands across), and downloads only when the
 * renderer first needs it — so glyphs route to the subset that covers them, never to a
 * same-named sibling that lacks them. Pass the result straight to `render`, which drops
 * the subsets the content doesn't use; or trim them yourself with {@link subsetFonts}.
 *
 * @example
 * fonts: await googleFont("Inter", { weight: [400, 700] })
 * @example
 * fonts: await googleFont("Inter", { weight: "100..900" }) // variable
 * @example
 * fonts: await googleFont("Inter", { weight: 700, style: "italic", text: "Hello" })
 */
export async function googleFont(
  family: string,
  options: GoogleFontOptions = {},
): Promise<FontSubset[]> {
  const css = await fetchCss(buildCssUrl(family, options), options);

  return parseSubsetFaces(css).map((face) => toSubset(face, options));
}

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
      // No `unicode-range` (e.g. a `text=` request) means the face covers everything.
      ranges: range ? parseUnicodeRange(range) : [],
    });
    index += 1;
  }

  return faces;
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

/** Drop the subsets `source` never renders, keeping each covering one and any range-less
 * fallback (a full font, or a `text=` Google subset). Pure — no network — so a renderer can
 * run it on the final node tree without re-fetching. Works on any descriptor carrying
 * `ranges`; entries without it are always kept. */
export function subsetFonts<T>(fonts: T[], source: string | Node | Node[]): T[] {
  const codepoints = collectCodepoints(source);

  return fonts.filter((font) =>
    rangesCover((font as { ranges?: [number, number][] }).ranges ?? [], codepoints),
  );
}

export type GoogleFontsOptions = FetchOptions & {
  /**
   * Cache for the Google Fonts CSS, keyed by request URL. Reuse one across renders (e.g. a
   * playground re-rendering on every edit) so the metadata is fetched and parsed only once.
   */
  cache?: Pick<Map<string, string>, "has" | "get" | "set">;
};

/** One css2 request for every family — Google returns all their subsets in a single CSS. */
function buildSubsetsUrl(specs: Exclude<GoogleFontFamily, string>[]) {
  const url = new URL(GOOGLE_FONTS_CSS);
  for (const spec of specs) {
    url.searchParams.append("family", familyValue(spec.family, spec));
  }
  return url.toString();
}

async function fetchSubsetsCss(url: string, options: GoogleFontsOptions) {
  const cached = options.cache?.get(url);
  if (cached !== undefined) {
    return cached;
  }

  const css = await fetchCss(url, options);
  options.cache?.set(url, css);
  return css;
}

/**
 * Load several Google Font families in ONE css2 request, returning every subset descriptor
 * (see {@link googleFont}). Hand the result to `render`, which keeps only the subsets the
 * content uses; or pre-trim with {@link subsetFonts}. Subsets download lazily, so the unused
 * ones cost nothing.
 *
 * @example
 * const fonts = await googleFonts(["Inter", "Noto Sans JP"]);
 * await render(element, { width, height, fonts });
 */
export async function googleFonts(
  families: GoogleFontFamily[],
  options: GoogleFontsOptions = {},
): Promise<FontSubset[]> {
  const specs = families.map((family) => (typeof family === "string" ? { family } : family));
  const css = await fetchSubsetsCss(buildSubsetsUrl(specs), options);

  return parseSubsetFaces(css).map((face) => toSubset(face, options));
}
