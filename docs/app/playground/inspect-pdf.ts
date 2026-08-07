/**
 * Reads back what a rendered PDF actually contains, so the playground can show
 * the structure instead of asking you to trust the render options. Page objects,
 * the outline, file specs and the XMP packet all sit outside object streams, so
 * this needs no PDF parser.
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
};

export type Bookmark = { title: string; depth: number };

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

export function inspectPdf(bytes: Uint8Array): PdfInspection {
  const text = bytesToChars(bytes);
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
  };
}
