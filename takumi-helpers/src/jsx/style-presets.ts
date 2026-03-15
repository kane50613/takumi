// Modified from https://github.com/vercel/satori/blob/2a0878a7f329bdba3a17ad68f71186a47add0dde/src/handler/presets.ts
// Reference from https://chromium.googlesource.com/chromium/blink/+/master/Source/core/css/html.css

import type { CSSProperties, JSX } from "react";

export const defaultStylePresets: Partial<
  Record<keyof JSX.IntrinsicElements, CSSProperties>
> = {
  html: {
    display: "block",
  },
  // children of the <head> element all have display: none
  head: {
    display: "none",
  },
  meta: {
    display: "none",
  },
  title: {
    display: "none",
  },
  link: {
    display: "none",
  },
  style: {
    display: "none",
  },
  script: {
    display: "none",
  },
  // Generic block-level elements
  body: {
    margin: 8,
    display: "block",
  },
  p: {
    marginTop: "1em",
    marginBottom: "1em",
    display: "block",
  },
  blockquote: {
    marginTop: "1em",
    marginBottom: "1em",
    marginLeft: 40,
    marginRight: 40,
    display: "block",
  },
  center: {
    textAlign: "center",
    display: "block",
  },
  hr: {
    marginTop: "0.5em",
    marginBottom: "0.5em",
    marginLeft: "auto",
    marginRight: "auto",
    borderWidth: 1,
    display: "block",
  },
  // Heading elements
  h1: {
    fontSize: "2em",
    marginTop: "0.67em",
    marginBottom: "0.67em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
  },
  h2: {
    fontSize: "1.5em",
    marginTop: "0.83em",
    marginBottom: "0.83em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
  },
  h3: {
    fontSize: "1.17em",
    marginTop: "1em",
    marginBottom: "1em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
  },
  h4: {
    marginTop: "1.33em",
    marginBottom: "1.33em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
  },
  h5: {
    fontSize: "0.83em",
    marginTop: "1.67em",
    marginBottom: "1.67em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
  },
  h6: {
    fontSize: "0.67em",
    marginTop: "2.33em",
    marginBottom: "2.33em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
  },
  u: {
    textDecoration: "underline",
  },
  strong: {
    fontWeight: "bold",
  },
  b: {
    fontWeight: "bold",
  },
  i: {
    fontStyle: "italic",
  },
  em: {
    fontStyle: "italic",
  },
  code: {
    fontFamily: "monospace",
  },
  kbd: {
    fontFamily: "monospace",
  },
  pre: {
    fontFamily: "monospace",
    margin: "1em 0",
    display: "block",
  },
  mark: {
    backgroundColor: "yellow",
    color: "black",
  },
  big: {
    fontSize: "1.2em",
  },
  small: {
    fontSize: "0.8em",
  },
  s: {
    textDecoration: "line-through",
  },
  div: {
    display: "block",
  },
};
