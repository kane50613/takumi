/**
 * Reads back what a rendered PDF actually contains, so the playground can show
 * the structure instead of asking you to trust the render options. Objects are
 * found by scanning for `N 0 obj` rather than through the cross-reference table,
 * which keeps this to one file and no PDF parser.
 */
export type PdfInspection = {
  pages: number;
  /** Conformance claimed by the XMP packet, e.g. `PDF/A-3b`. */
  standards: string[];
  tagged: boolean;
  title?: string;
  authors?: string[];
  created?: string;
  bookmarks: Bookmark[];
  attachments: { name: string; description?: string }[];
  /**
   * One shaped run emits one `BT` block, so a page with about as many blocks as
   * words is drawing every word separately and extractors will lose the spaces.
   */
  pageText: { blocks: number; words: number }[];
  objects: PdfObject[];
};

/**
 * One indirect object, with its stream decoded when the bytes read as text.
 * `text` is a content stream's glyph codes read back through `/ToUnicode`.
 */
export type PdfObject = {
  number: string;
  label: string;
  dict: string;
  body?: string;
  text?: string;
};

type Bookmark = { title: string; depth: number };

const BODY_LIMIT = 64 * 1024;

/**
 * One char per byte. `TextDecoder("latin1")` is windows-1252 by spec, which
 * rewrites the 0x80–0x9f bytes that UTF-8 text is full of.
 */
function bytesToChars(bytes: Uint8Array): string {
  let text = "";

  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }

  return text;
}

/** Re-reads a slice of the byte-per-char file as the UTF-8 the XMP packet is written in. */
function utf8(value: string): string;
function utf8(value: string | undefined): string | undefined;
function utf8(value: string | undefined) {
  return value && new TextDecoder().decode(Uint8Array.from(value, (char) => char.charCodeAt(0)));
}

/** Decodes a PDF literal `(string)` or a UTF-16BE `<hex string>`. */
function pdfString(raw: string): string {
  if (raw.startsWith("<")) {
    const hex = raw.slice(1, -1).replace(/\s/g, "");
    const text = (hex.match(/.{4}/g) ?? [])
      .map((unit) => String.fromCharCode(Number.parseInt(unit, 16)))
      .join("");

    return text.replace(/^﻿/, "");
  }

  return raw.slice(1, -1).replace(/\\([()\\])/g, "$1");
}

function entry(object: string, key: string): string | undefined {
  const match = object.match(new RegExp(`/${key}\\s*(\\([^)]*\\)|<[0-9A-Fa-f\\s]*>)`));
  return match?.[1] && pdfString(match[1]);
}

/** Walks the outline's `/First` and `/Next` chain, which holds the reading order. */
function readBookmarks(objects: string[]): Bookmark[] {
  const byNumber = new Map<string, string>();

  for (const object of objects) {
    const number = object.match(/(\d+)\s+0\s+obj/)?.[1];
    if (number) byNumber.set(number, object);
  }

  const followRef = (object: string | undefined, key: string) => {
    const number = object?.match(new RegExp(`/${key}\\s+(\\d+)\\s+0\\s+R`))?.[1];
    return number === undefined ? undefined : byNumber.get(number);
  };

  const bookmarks: Bookmark[] = [];
  const walk = (first: string | undefined, depth: number) => {
    let node = first;

    while (node && bookmarks.length < 500) {
      bookmarks.push({ title: entry(node, "Title") ?? "", depth });
      walk(followRef(node, "First"), depth + 1);
      node = followRef(node, "Next");
    }
  };

  walk(
    followRef(
      objects.find((object) => /\/Type\s*\/Outlines/.test(object)),
      "First",
    ),
    0,
  );

  return bookmarks.filter((bookmark) => bookmark.title);
}

/** Every indirect object's body, keyed by number. One char per byte, so the offsets index both. */
function objectRanges(text: string): Map<string, [number, number]> {
  const ranges = new Map<string, [number, number]>();

  for (const match of text.matchAll(/(\d+)\s+0\s+obj/g)) {
    const start = match.index + match[0].length;
    const end = text.indexOf("endobj", start);

    ranges.set(match[1], [start, end === -1 ? text.length : end]);
  }

  return ranges;
}

function references(value: string | undefined): string[] {
  return [...(value ?? "").matchAll(/(\d+)\s+0\s+R/g)].map((match) => match[1]);
}

function printable(data: Uint8Array): string {
  return bytesToChars(data).replace(
    /[^\n\x20-\x7e]/g,
    (char) => `\\x${char.charCodeAt(0).toString(16).padStart(2, "0")}`,
  );
}

async function inflate(data: Uint8Array): Promise<Uint8Array> {
  const stream = new Blob([data.slice()]).stream().pipeThrough(new DecompressionStream("deflate"));

  return new Uint8Array(await new Response(stream).arrayBuffer());
}

async function readStream(
  text: string,
  bytes: Uint8Array,
  [start, end]: [number, number],
): Promise<Uint8Array | undefined> {
  const object = text.slice(start, end);
  const keyword = object.indexOf("stream");
  const close = object.lastIndexOf("endstream");

  if (keyword === -1 || close === -1) return undefined;

  const from = start + keyword + (object.startsWith("\r\n", keyword + 6) ? 8 : 7);
  const dict = object.slice(0, keyword);
  const length = Number(dict.match(/\/Length\s+(\d+)/)?.[1]);
  let to = start + close;

  // Only trust the `endstream` offset when `/Length` is indirect: compressed
  // data ending in 0x0a is real, and trimming it corrupts the stream.
  if (Number.isInteger(length) && from + length <= to) to = from + length;
  else while (to > from && (bytes[to - 1] === 0x0a || bytes[to - 1] === 0x0d)) to -= 1;

  const raw = bytes.subarray(from, to);

  try {
    return /\/FlateDecode/.test(dict) ? await inflate(raw) : raw;
  } catch {
    return undefined;
  }
}

function clamp(value: string): string {
  return value.length > BODY_LIMIT ? `${value.slice(0, BODY_LIMIT)}\n… truncated` : value;
}

/** Font programs and images escape into several chars per byte, which is not worth reading. */
function readable(data: Uint8Array): string | undefined {
  const escaped = printable(data);

  return escaped.length > data.length * 2 ? undefined : escaped;
}

/** A dictionary entry's raw value: a nested dictionary, or the reference standing in for one. */
function dictValue(dict: string, key: string): string | undefined {
  const at = dict.search(new RegExp(`/${key}\\b`));

  if (at === -1) return undefined;

  const rest = dict.slice(at + key.length + 1).trimStart();

  if (!rest.startsWith("<<")) return rest.match(/^\d+\s+0\s+R/)?.[0];

  let depth = 0;

  for (let index = 0; index < rest.length; index += 1) {
    if (rest.startsWith("<<", index)) depth += 1;
    else if (rest.startsWith(">>", index)) {
      depth -= 1;
      if (depth === 0) return rest.slice(0, index + 2);
    } else continue;

    index += 1;
  }

  return undefined;
}

/** A CMap destination is UTF-16BE, so an emoji arrives as the two units of its surrogate pair. */
function utf16Units(hex: string): number[] {
  return (hex.match(/.{4}/g) ?? []).map((unit) => Number.parseInt(unit, 16));
}

/** Reads `beginbfchar` and `beginbfrange`, the two forms Takumi's CMaps use. */
function parseCMap(cmap: string): Map<number, string> {
  const codes = new Map<number, string>();

  for (const block of cmap.matchAll(/beginbfchar([\s\S]*?)endbfchar/g)) {
    for (const [, code, value] of block[1].matchAll(/<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>/g)) {
      codes.set(Number.parseInt(code, 16), String.fromCharCode(...utf16Units(value)));
    }
  }

  // The `[ <a> <b> ]` destination form is skipped: Takumi never writes it.
  for (const block of cmap.matchAll(/beginbfrange([\s\S]*?)endbfrange/g)) {
    const entries = block[1].matchAll(/<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>/g);

    for (const [, low, high, value] of entries) {
      const start = Number.parseInt(low, 16);
      const units = utf16Units(value);

      if (units.length === 0) continue;

      // A range steps the last unit, which is what keeps a surrogate pair intact.
      for (let code = start; code <= Number.parseInt(high, 16); code += 1) {
        const stepped = units.with(-1, units[units.length - 1] + code - start);

        codes.set(code, String.fromCharCode(...stepped));
      }
    }
  }

  return codes;
}

/** Undoes the `\ooo` and `\n` escapes a PDF literal string is written with. */
function pdfLiteral(raw: string): string {
  return raw.replace(/\\(\d{1,3}|[\s\S])/g, (_, escape: string) =>
    /^\d/.test(escape)
      ? String.fromCharCode(Number.parseInt(escape, 8))
      : ({ n: "\n", r: "\r", t: "\t", b: "\b", f: "\f" }[escape] ?? escape),
  );
}

/**
 * The words a content stream draws. Codes are two bytes because every font
 * Takumi embeds is Identity-encoded, and `Tf` says which CMap reads them.
 */
function extractText(content: string, fonts: Map<string, Map<number, string>>): string {
  const tokens = content.matchAll(
    /\/(\w+)\s+[\d.]+\s+Tf|\(((?:\\[\s\S]|[^\\()])*)\)|<([0-9A-Fa-f\s]+)>/g,
  );

  let cmap: Map<number, string> | undefined;
  let out = "";

  for (const [, font, literal, hex] of tokens) {
    if (font !== undefined) {
      cmap = fonts.get(font);
      continue;
    }

    if (!cmap) continue;

    for (const code of showCodes(literal, hex)) out += cmap.get(code) ?? "";
  }

  return out;
}

/** Both string forms carry the same two-byte codes, written differently. */
function showCodes(literal: string | undefined, hex: string | undefined): number[] {
  if (literal === undefined) {
    return ((hex ?? "").replace(/\s/g, "").match(/.{4}/g) ?? []).map((unit) =>
      Number.parseInt(unit, 16),
    );
  }

  const bytes = pdfLiteral(literal);

  return Array.from(
    { length: bytes.length >> 1 },
    (_, index) => (bytes.charCodeAt(index * 2) << 8) | bytes.charCodeAt(index * 2 + 1),
  );
}

/** Objects packed into an `/ObjStm`, which the file never lists at the top level. */
function expandObjectStream(dict: string, data: Uint8Array): PdfObject[] {
  const first = Number(dict.match(/\/First\s+(\d+)/)?.[1]);
  const count = Number(dict.match(/\/N\s+(\d+)/)?.[1]);

  if (!Number.isInteger(first) || !Number.isInteger(count)) return [];

  const text = bytesToChars(data);
  const header = text.slice(0, first).trim().split(/\s+/).map(Number);

  return Array.from({ length: count }, (_, index) => {
    const start = first + header[index * 2 + 1];
    const next = header[index * 2 + 3];
    const source = text.slice(start, next === undefined ? text.length : first + next).trim();

    return {
      number: String(header[index * 2]),
      label: source.match(/\/(?:Sub)?Type\s*\/(\w+)/)?.[1] ?? "",
      dict: source,
    };
  });
}

/** The `/Font` resources a page names, each paired with the CMap that reads its codes. */
function pageFonts(
  page: string,
  source: (number: string) => string | undefined,
  streams: Map<string, Uint8Array>,
): Map<string, Map<number, string>> {
  const resolve = (value: string | undefined) => {
    const reference = value?.match(/^(\d+)\s+0\s+R/)?.[1];
    return reference ? source(reference) : value;
  };

  const resources = resolve(dictValue(page, "Resources")) ?? "";
  const fonts = resolve(dictValue(resources, "Font")) ?? "";
  const cmaps = new Map<string, Map<number, string>>();

  for (const [, name, reference] of fonts.matchAll(/\/(\w+)\s+(\d+)\s+0\s+R/g)) {
    const toUnicode = dictValue(source(reference) ?? "", "ToUnicode")?.match(/^\d+/)?.[0];
    const cmap = toUnicode && streams.get(toUnicode);

    if (cmap) cmaps.set(name, parseCMap(bytesToChars(cmap)));
  }

  return cmaps;
}

type ObjectSource = {
  number: string;
  raw: string;
  stream: Uint8Array | undefined;
  body: string | undefined;
  pages: number[] | undefined;
  text: string | undefined;
};

/** The object itself, followed by whatever an `/ObjStm` was hiding inside it. */
function describeObject({ number, raw, stream, body, pages, text }: ObjectSource): PdfObject[] {
  const keyword = raw.indexOf("stream");
  const dict = (keyword === -1 ? raw : raw.slice(0, keyword)).trim();
  const packed = stream && /\/Type\s*\/ObjStm\b/.test(dict) ? expandObjectStream(dict, stream) : [];

  return [
    {
      number,
      label: pages
        ? `content stream, page${pages.length > 1 ? "s" : ""} ${pages.join(", ")}`
        : (raw.match(/\/(?:Sub)?Type\s*\/(\w+)/)?.[1] ?? ""),
      dict,
      body:
        packed.length > 0
          ? undefined
          : stream && (body === undefined ? `${stream.length} bytes, not text` : clamp(body)),
      text,
    },
    ...packed,
  ];
}

function countPageText(
  pageText: { blocks: number; words: number }[],
  pages: number[],
  body: string,
  text: string,
) {
  const blocks = body.match(/\bBT\b/g)?.length ?? 0;
  const words = text.split(/\s+/).filter(Boolean).length;

  for (const page of pages) {
    pageText[page - 1].blocks += blocks;
    pageText[page - 1].words += words;
  }
}

/** Every object the file declares, in file order, with `/ObjStm` contents unpacked in place. */
async function readObjects(text: string, bytes: Uint8Array) {
  const ranges = objectRanges(text);
  const source = (number: string) => {
    const range = ranges.get(number);
    return range && text.slice(...range);
  };

  // Object numbers do not carry page order; `/Kids` does.
  const tree = [...ranges.keys()].find((number) => /\/Type\s*\/Pages\b/.test(source(number) ?? ""));
  const kids = references(source(tree ?? "")?.match(/\/Kids\s*\[([^\]]*)\]/)?.[1]);
  // Streams are decoded up front because a content stream is only readable
  // through the `/ToUnicode` stream of a font two references away.
  const streams = new Map<string, Uint8Array>();

  await Promise.all(
    [...ranges].map(async ([number, range]) => {
      const stream = await readStream(text, bytes, range);

      if (stream) streams.set(number, stream);
    }),
  );

  const pageOf = new Map<string, number[]>();
  const fontsOf = new Map<string, Map<string, Map<number, string>>>();

  kids.forEach((kid, index) => {
    const page = source(kid) ?? "";
    const contents = references(page.match(/\/Contents\s*(\[[^\]]*\]|\d+\s+0\s+R)/)?.[1]);
    const fonts = pageFonts(page, source, streams);

    // Two pages may share one stream, so a page number appends instead of replacing.
    for (const number of contents) {
      pageOf.set(number, [...(pageOf.get(number) ?? []), index + 1]);
      fontsOf.set(number, fonts);
    }
  });

  const pageText = kids.map(() => ({ blocks: 0, words: 0 }));
  const objects = [...ranges].flatMap(([number, range]) => {
    const stream = streams.get(number);
    const body = stream && readable(stream);
    const pages = pageOf.get(number);
    const drawn =
      pages && stream
        ? clamp(extractText(bytesToChars(stream), fontsOf.get(number) ?? new Map()))
        : undefined;

    if (pages && body) countPageText(pageText, pages, body, drawn ?? "");

    return describeObject({ number, raw: text.slice(...range), stream, body, pages, text: drawn });
  });

  return { objects, pageText };
}

export async function inspectPdf(bytes: Uint8Array): Promise<PdfInspection> {
  const text = bytesToChars(bytes);
  const { objects: indirect, pageText } = await readObjects(text, bytes);
  const objects = text.split("endobj");
  const xmp = text.match(/<x:xmpmeta[\s\S]*?<\/x:xmpmeta>/)?.[0] ?? "";
  const xmpValue = (tag: string) => xmp.match(new RegExp(`<${tag}>([^<]*)<`))?.[1];

  const archival = xmpValue("pdfaid:part");
  const accessible = xmpValue("pdfuaid:part");
  const standards = [
    archival && `PDF/A-${archival}${(xmpValue("pdfaid:conformance") ?? "").toLowerCase()}`,
    accessible && `PDF/UA-${accessible}`,
  ].filter((standard): standard is string => Boolean(standard));

  const authors = [...xmp.matchAll(/<dc:creator>[\s\S]*?<\/dc:creator>/g)]
    .flatMap((match) => [...match[0].matchAll(/<rdf:li>([^<]*)</g)])
    .map((match) => utf8(match[1]));

  return {
    pages: text.match(/\/Type\s*\/Page[^s]/g)?.length ?? 0,
    standards,
    tagged: text.includes("/StructTreeRoot"),
    title: utf8(xmp.match(/<dc:title>[\s\S]*?<rdf:li[^>]*>([^<]*)</)?.[1]),
    authors: authors.length > 0 ? authors : undefined,
    created: xmpValue("xmp:CreateDate")?.slice(0, 10),
    bookmarks: readBookmarks(objects),
    attachments: objects
      .filter((object) => object.includes("/Filespec"))
      .map((object) => ({ name: entry(object, "F") ?? "", description: entry(object, "Desc") }))
      .filter((attachment) => attachment.name),
    pageText,
    objects: indirect,
  };
}
