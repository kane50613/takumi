// Chrome UA so the Google Fonts CSS API returns `woff2` `src` URLs.
const chromeUserAgent =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export type FontSource = string | URL | ArrayBuffer | Uint8Array;

export type FontDescriptor = {
  /** Font family name matched against `fontFamily`. Falls back to the name in the font file. */
  name?: string;
  /** Font weight. Falls back to the weight in the font file. */
  weight?: number;
  /** Font style, e.g. `"normal"` or `"italic"`. Falls back to the style in the font file. */
  style?: string;
};

/** A resolved font descriptor, ready to pass to `ImageResponse`/`Renderer` `fonts`. */
export type FontData = FontDescriptor & {
  data: ArrayBuffer | Uint8Array;
};

const bytesCache = new Map<string, Promise<ArrayBuffer>>();
const cssCache = new Map<string, Promise<string>>();

function fetchBytes(url: string, fetchImpl: FetchLike): Promise<ArrayBuffer> {
  const cached = bytesCache.get(url);
  if (cached) {
    return cached;
  }

  const pending = fetchImpl(url).then((response) => {
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} fetching font: ${url}`);
    }
    return response.arrayBuffer();
  });

  bytesCache.set(url, pending);
  return pending;
}

async function resolveSource(
  source: FontSource,
  fetchImpl: FetchLike,
): Promise<ArrayBuffer | Uint8Array> {
  if (source instanceof Uint8Array || source instanceof ArrayBuffer) {
    return source;
  }

  const location = source instanceof URL ? source.href : source;

  if (/^https?:\/\//.test(location)) {
    return fetchBytes(location, fetchImpl);
  }

  // Local filesystem path (Node-only); imported lazily so edge/WASM bundles stay clean.
  const { readFile } = await import("node:fs/promises");
  return readFile(location);
}

export type FontOptions = {
  /** Custom fetch implementation. @default globalThis.fetch */
  fetch?: FetchLike;
};

/**
 * Resolve a single font from a URL, filesystem path, or raw bytes into a descriptor.
 *
 * @example
 * fonts: [await font("./Inter.ttf", { name: "Inter", weight: 400 })]
 */
export async function font(
  source: FontSource,
  descriptor: FontDescriptor = {},
  options: FontOptions = {},
): Promise<FontData> {
  const data = await resolveSource(source, options.fetch ?? globalThis.fetch);
  return { ...descriptor, data };
}

export type GoogleFontOptions = FontOptions & {
  /** Weights to fetch. @default [400] */
  weights?: number[];
  /** Styles to fetch. @default ["normal"] */
  styles?: ("normal" | "italic")[];
  /** Restrict the download to the glyphs needed for this text. */
  text?: string;
  /** `font-display` strategy passed through to the CSS request. */
  display?: "auto" | "block" | "swap" | "fallback" | "optional";
};

function buildCssUrl(family: string, options: GoogleFontOptions): string {
  const weights = options.weights ?? [400];
  const styles = options.styles ?? ["normal"];

  let axis: string;
  if (styles.includes("italic")) {
    const italics = styles.includes("normal") ? [0, 1] : [1];
    axis = `ital,wght@${italics
      .flatMap((ital) => weights.map((weight) => `${ital},${weight}`))
      .sort()
      .join(";")}`;
  } else {
    axis = `wght@${[...weights].sort((a, b) => a - b).join(";")}`;
  }

  let url = `https://fonts.googleapis.com/css2?family=${family.replace(/ /g, "+")}:${axis}`;
  if (options.display) {
    url += `&display=${options.display}`;
  }
  if (options.text) {
    url += `&text=${encodeURIComponent(options.text)}`;
  }
  return url;
}

function fetchCss(url: string, fetchImpl: FetchLike): Promise<string> {
  const cached = cssCache.get(url);
  if (cached) {
    return cached;
  }

  const pending = fetchImpl(url, { headers: { "User-Agent": chromeUserAgent } }).then(
    (response) => {
      if (!response.ok) {
        throw new Error(`HTTP ${response.status} fetching Google Fonts CSS: ${url}`);
      }
      return response.text();
    },
  );

  cssCache.set(url, pending);
  return pending;
}

const fontFacePattern = /@font-face\s*\{([^}]*)\}/g;

function parseFontFaces(css: string): { url: string; weight?: number; style?: string }[] {
  const faces: { url: string; weight?: number; style?: string }[] = [];
  const seen = new Set<string>();

  for (const [, body] of css.matchAll(fontFacePattern)) {
    const url = body
      .match(/src:\s*url\(([^)]+)\)/)?.[1]
      ?.replace(/['"]/g, "")
      .trim();
    if (!url || seen.has(url)) {
      continue;
    }
    seen.add(url);

    const weight = body.match(/font-weight:\s*(\d+)/)?.[1];
    faces.push({
      url,
      weight: weight ? Number(weight) : undefined,
      style: body.match(/font-style:\s*([a-z]+)/i)?.[1],
    });
  }

  return faces;
}

/**
 * Resolve a Google Font into ready-to-use descriptors — handles the CSS lookup, `woff2`
 * URL extraction, and multi-weight expansion that OG repos otherwise hand-roll.
 *
 * @example
 * fonts: await googleFont("Inter", { weights: [400, 700] })
 */
export async function googleFont(
  family: string,
  options: GoogleFontOptions = {},
): Promise<FontData[]> {
  const fetchImpl = options.fetch ?? globalThis.fetch;
  const css = await fetchCss(buildCssUrl(family, options), fetchImpl);

  return Promise.all(
    parseFontFaces(css).map(async (face) => ({
      name: family,
      data: await fetchBytes(face.url, fetchImpl),
      weight: face.weight,
      style: face.style,
    })),
  );
}
