import { ProsePage } from "~/components/prose-page";

const TITLE = "About Takumi";
const DESCRIPTION =
  "Takumi is an open source rendering engine that turns JSX, HTML, and node trees into images, SVG, and PDF from Rust.";

export default function About() {
  return (
    <ProsePage title={TITLE} heading="About Takumi" description={DESCRIPTION} path="/about">
      <p>
        Takumi renders a layout you describe in JSX, HTML, or a plain node tree, then writes it out
        as PNG, WebP, AVIF, JPEG, SVG, or PDF. The layout, text shaping, and painting all happen in
        Rust. There is no headless Chrome to install, keep warm, or pay for.
      </p>
      <h2>What it is for</h2>
      <p>
        People use Takumi for open graph images, social cards, certificates, receipts, invoices,
        reports, and tickets. It runs on a server that generates a card per request, and it runs in
        a build step that generates thousands at once. The same template also compiles to PDF, so
        one component can produce both the preview image and the printable document.
      </p>
      <h2>Where it runs</h2>
      <p>
        The engine ships as a Rust crate, as a native Node addon, and as a WebAssembly module. That
        covers Node, Bun, Deno, Cloudflare Workers, and the browser. The{" "}
        <a href="/playground">playground</a> runs the WebAssembly build inside your tab, which is
        the same code path a Worker uses.
      </p>
      <h2>Who maintains it</h2>
      <p>
        Takumi is written and maintained by Kane Wang in Taiwan, with help from contributors on
        GitHub. The source lives at{" "}
        <a href="https://github.com/kane50613/takumi">github.com/kane50613/takumi</a> and is
        licensed under MIT or Apache-2.0, at your option. Issues, feature requests, and pull
        requests are all handled in that repository.
      </p>
      <h2>For agents</h2>
      <p>
        The documentation is published as Markdown as well as HTML. Read{" "}
        <a href="/llms.txt">/llms.txt</a> for an outline,{" "}
        <a href="/llms-full.txt">/llms-full.txt</a> for the whole thing in one file, and{" "}
        <a href="/openapi.json">/openapi.json</a> for the endpoints this site serves. Any
        documentation URL returns Markdown when the request asks for <code>text/markdown</code>.
      </p>
    </ProsePage>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
