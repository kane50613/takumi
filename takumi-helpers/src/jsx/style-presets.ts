// Modified from https://github.com/vercel/satori/blob/2a0878a7f329bdba3a17ad68f71186a47add0dde/src/handler/presets.ts
// Reference from https://chromium.googlesource.com/chromium/blink/+/master/Source/core/css/html.css

import type { CSSProperties, JSX } from "react";

export const defaultStylePresets: Partial<Record<keyof JSX.IntrinsicElements, CSSProperties>> = {
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
  noscript: {
    display: "none",
  },
  datalist: {
    display: "none",
  },
  template: {
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
    unicodeBidi: "isolate",
  },
  blockquote: {
    marginTop: "1em",
    marginBottom: "1em",
    marginLeft: 40,
    marginRight: 40,
    display: "block",
    unicodeBidi: "isolate",
  },
  figure: {
    marginTop: "1em",
    marginBottom: "1em",
    marginLeft: 40,
    marginRight: 40,
    display: "block",
    unicodeBidi: "isolate",
  },
  figcaption: {
    display: "block",
    unicodeBidi: "isolate",
  },
  address: {
    fontStyle: "italic",
    display: "block",
    unicodeBidi: "isolate",
  },
  article: {
    display: "block",
    unicodeBidi: "isolate",
  },
  aside: {
    display: "block",
    unicodeBidi: "isolate",
  },
  footer: {
    display: "block",
    unicodeBidi: "isolate",
  },
  header: {
    display: "block",
    unicodeBidi: "isolate",
  },
  hgroup: {
    display: "block",
    unicodeBidi: "isolate",
  },
  main: {
    display: "block",
    unicodeBidi: "isolate",
  },
  nav: {
    display: "block",
    unicodeBidi: "isolate",
  },
  section: {
    display: "block",
    unicodeBidi: "isolate",
  },
  center: {
    textAlign: "center",
    display: "block",
    unicodeBidi: "isolate",
  },
  hr: {
    marginTop: "0.5em",
    marginBottom: "0.5em",
    marginLeft: "auto",
    marginRight: "auto",
    borderWidth: 1,
    display: "block",
    unicodeBidi: "isolate",
  },
  // Lists
  ul: {
    marginTop: "1em",
    marginBottom: "1em",
    paddingInlineStart: 40,
    display: "block",
    listStyleType: "disc",
    unicodeBidi: "isolate",
  },
  ol: {
    marginTop: "1em",
    marginBottom: "1em",
    paddingInlineStart: 40,
    display: "block",
    listStyleType: "decimal",
    unicodeBidi: "isolate",
  },
  menu: {
    marginTop: "1em",
    marginBottom: "1em",
    paddingInlineStart: 40,
    display: "block",
    listStyleType: "disc",
    unicodeBidi: "isolate",
  },
  li: {
    display: "list-item",
    unicodeBidi: "isolate",
  },
  dl: {
    marginTop: "1em",
    marginBottom: "1em",
    display: "block",
    unicodeBidi: "isolate",
  },
  dt: {
    display: "block",
    unicodeBidi: "isolate",
  },
  dd: {
    marginLeft: 40,
    display: "block",
    unicodeBidi: "isolate",
  },
  // Forms and interactive elements
  form: {
    display: "block",
    unicodeBidi: "isolate",
  },
  fieldset: {
    marginLeft: 2,
    marginRight: 2,
    paddingTop: "0.35em",
    paddingRight: "0.75em",
    paddingBottom: "0.625em",
    paddingLeft: "0.75em",
    borderWidth: 2,
    display: "block",
  },
  legend: {
    paddingLeft: 2,
    paddingRight: 2,
    display: "block",
    unicodeBidi: "isolate",
  },
  details: {
    display: "block",
  },
  summary: {
    display: "block",
    unicodeBidi: "isolate",
  },
  search: {
    display: "block",
    unicodeBidi: "isolate",
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
    unicodeBidi: "isolate",
  },
  h2: {
    fontSize: "1.5em",
    marginTop: "0.83em",
    marginBottom: "0.83em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
    unicodeBidi: "isolate",
  },
  h3: {
    fontSize: "1.17em",
    marginTop: "1em",
    marginBottom: "1em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
    unicodeBidi: "isolate",
  },
  h4: {
    marginTop: "1.33em",
    marginBottom: "1.33em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
    unicodeBidi: "isolate",
  },
  h5: {
    fontSize: "0.83em",
    marginTop: "1.67em",
    marginBottom: "1.67em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
    unicodeBidi: "isolate",
  },
  h6: {
    fontSize: "0.67em",
    marginTop: "2.33em",
    marginBottom: "2.33em",
    marginLeft: 0,
    marginRight: 0,
    fontWeight: "bold",
    display: "block",
    unicodeBidi: "isolate",
  },
  u: {
    textDecoration: "underline",
  },
  ins: {
    textDecoration: "underline",
  },
  strong: {
    fontWeight: "bolder",
  },
  b: {
    fontWeight: "bolder",
  },
  i: {
    fontStyle: "italic",
  },
  em: {
    fontStyle: "italic",
  },
  cite: {
    fontStyle: "italic",
  },
  dfn: {
    fontStyle: "italic",
  },
  code: {
    fontFamily: "monospace",
  },
  kbd: {
    fontFamily: "monospace",
  },
  samp: {
    fontFamily: "monospace",
  },
  pre: {
    fontFamily: "monospace",
    whiteSpace: "pre",
    margin: "1em 0",
    display: "block",
    unicodeBidi: "isolate",
  },
  br: {
    whiteSpace: "pre",
  },
  mark: {
    backgroundColor: "yellow",
    color: "black",
  },
  big: {
    fontSize: "larger",
  },
  small: {
    fontSize: "smaller",
  },
  s: {
    textDecoration: "line-through",
  },
  del: {
    textDecoration: "line-through",
  },
  sub: {
    fontSize: "smaller",
    verticalAlign: "sub",
  },
  sup: {
    fontSize: "smaller",
    verticalAlign: "super",
  },
  div: {
    display: "block",
    unicodeBidi: "isolate",
  },
  table: {
    display: "table",
    boxSizing: "border-box",
    borderSpacing: "2px",
    unicodeBidi: "isolate",
  },
  thead: {
    display: "table-header-group",
    breakInside: "avoid",
    unicodeBidi: "isolate",
  },
  tbody: {
    display: "table-row-group",
    unicodeBidi: "isolate",
  },
  tfoot: {
    display: "table-footer-group",
    breakInside: "avoid",
    unicodeBidi: "isolate",
  },
  tr: {
    display: "table-row",
    unicodeBidi: "isolate",
  },
  td: {
    display: "table-cell",
    padding: 1,
    unicodeBidi: "isolate",
  },
  th: {
    display: "table-cell",
    padding: 1,
    fontWeight: "bold",
    textAlign: "center",
    unicodeBidi: "isolate",
  },
  caption: {
    display: "table-caption",
    textAlign: "center",
    unicodeBidi: "isolate",
  },
  colgroup: {
    display: "table-column-group",
    unicodeBidi: "isolate",
  },
  col: {
    display: "table-column",
    unicodeBidi: "isolate",
  },
  bdi: {
    unicodeBidi: "isolate",
  },
  bdo: {
    unicodeBidi: "isolate-override",
  },
  output: {
    unicodeBidi: "isolate",
  },
};
