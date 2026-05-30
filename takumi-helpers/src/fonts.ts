// Chrome UA so the Google Fonts CSS API returns `woff2` `src` URLs.
const chromeUserAgent =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export type FontOptions = {
  /** Custom fetch implementation. @default globalThis.fetch */
  fetch?: FetchLike;
  /** Abort each underlying request after this many milliseconds. */
  timeout?: number;
};

function withTimeout(init: RequestInit, timeout?: number): RequestInit {
  return timeout === undefined ? init : { ...init, signal: AbortSignal.timeout(timeout) };
}

async function fetchBytes(url: string, fetchImpl: FetchLike, timeout?: number) {
  const response = await fetchImpl(url, withTimeout({}, timeout));
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} fetching font: ${url}`);
  }
  return response.arrayBuffer();
}

type FontStyle = "normal" | "italic";

export type GoogleFontOptions = FontOptions & {
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

async function fetchCss(url: string, fetchImpl: FetchLike, timeout?: number) {
  const response = await fetchImpl(
    url,
    withTimeout({ headers: { "User-Agent": chromeUserAgent } }, timeout),
  );
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} fetching Google Fonts CSS: ${url}`);
  }
  return response.text();
}

const fontFacePattern = /@font-face\s*\{([^}]*)\}/g;

function parseFontFaces(css: string) {
  const faces: { url: string; weight?: number; style?: string }[] = [];
  const seen = new Set<string>();

  for (const match of css.matchAll(fontFacePattern)) {
    const body = match[1];
    if (!body) {
      continue;
    }

    const url = body
      .match(/src:\s*url\(([^)]+)\)/)?.[1]
      ?.replace(/['"]/g, "")
      .trim();
    if (!url || seen.has(url)) {
      continue;
    }
    seen.add(url);

    // A range like `100 900` means a variable file. Leave weight unset so CSS controls it.
    const weight = body.match(/font-weight:\s*(\d+)(?:\s+(\d+))?/);
    faces.push({
      url,
      weight: weight && !weight[2] ? Number(weight[1]) : undefined,
      style: body.match(/font-style:\s*([a-z]+)/i)?.[1],
    });
  }

  return faces;
}

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
  const fetchImpl = options.fetch ?? globalThis.fetch;
  const css = await fetchCss(buildCssUrl(family, options), fetchImpl, options.timeout);

  return parseFontFaces(css).map((face) => ({
    name: family,
    key: face.url,
    weight: face.weight,
    style: face.style,
    data: () => fetchBytes(face.url, fetchImpl, options.timeout),
  }));
}
