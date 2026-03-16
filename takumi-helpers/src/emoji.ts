import { container, image, text } from "./helpers";
import type { Node, TextNode } from "./types";

export type EmojiType = "twemoji" | "blobmoji" | "noto" | "openmoji";

const UFE0Fg = /\uFE0F/g;
const U200D = String.fromCharCode(0x200d);

function getIconCode(char: string) {
  const c = char.indexOf(U200D) < 0 ? char.replace(UFE0Fg, "") : char;
  let r = "";
  for (let i = 0, p = 0; i < c.length; i++) {
    const cc = c.charCodeAt(i);
    if (p) {
      const code = (65536 + ((p - 55296) << 10) + (cc - 56320)).toString(16);
      r += (r ? "-" : "") + code;
      p = 0;
    } else if (55296 <= cc && cc <= 56319) {
      p = cc;
    } else {
      r += (r ? "-" : "") + cc.toString(16);
    }
  }
  return r;
}

const apis = {
  twemoji: (code: string) =>
    `https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/svg/${code.toLowerCase()}.svg`,
  openmoji: "https://cdn.jsdelivr.net/npm/@svgmoji/openmoji@2.0.0/svg/",
  blobmoji: "https://cdn.jsdelivr.net/npm/@svgmoji/blob@2.0.0/svg/",
  noto: "https://cdn.jsdelivr.net/gh/svgmoji/svgmoji/packages/svgmoji__noto/svg/",
};

function getEmojiUrl(icon: string, type: EmojiType) {
  const code = getIconCode(icon);
  return type === "twemoji"
    ? apis.twemoji(code)
    : `${apis[type]}${code.toUpperCase()}.svg`;
}

let segmenter: Intl.Segmenter | null | undefined;

function getSegmenter(): Intl.Segmenter | null {
  if (segmenter === undefined) {
    if (typeof Intl !== "undefined" && typeof Intl.Segmenter === "function") {
      segmenter = new Intl.Segmenter("en", { granularity: "grapheme" });
    } else {
      segmenter = null;
    }
  }
  return segmenter;
}

function getSegments(text: string): { segment: string }[] {
  const s = getSegmenter();
  if (s) {
    return Array.from(s.segment(text));
  }
  return Array.from(text).map((s) => ({ segment: s }));
}

function splitTextToNodes(node: TextNode, emojiType: EmojiType): Node[] {
  const nodes: Node[] = [];
  let currentText = "";

  const segments = getSegments(node.text);

  for (const { segment } of segments) {
    if (/\p{Extended_Pictographic}/u.test(segment)) {
      if (currentText) {
        nodes.push(text({ text: currentText }));
        currentText = "";
      }
      nodes.push(
        image({
          src: getEmojiUrl(segment, emojiType),
          style: {
            display: "inline-block",
            width: "1em",
            height: "1em",
            margin: "0 0.05em 0 0.1em",
            verticalAlign: "-0.1em",
          },
        }),
      );
    } else {
      currentText += segment;
    }
  }

  if (currentText) {
    nodes.push(text({ text: currentText }));
  }

  return nodes;
}

export function extractEmojis(node: Node, emojiType: EmojiType): Node {
  if (node.type === "text") {
    const hasEmoji = getSegments(node.text).some(({ segment }) =>
      /\p{Extended_Pictographic}/u.test(segment),
    );

    if (hasEmoji) {
      const { type: _, ...metadata } = node;
      return container({
        ...metadata,
        children: splitTextToNodes(node, emojiType),
      });
    }
  } else if (node.type === "container" && node.children) {
    return {
      ...node,
      children: node.children.map((child) =>
        child ? extractEmojis(child, emojiType) : child,
      ),
    };
  }

  return node;
}
