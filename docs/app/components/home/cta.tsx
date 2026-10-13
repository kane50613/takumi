import { Link } from "waku";
import { Code, H2, LEAD, LINK, Section, TEXT_COLUMN, VISUAL_COLUMN } from "./section";
import type { ThemedHtml } from "./shiki-html";

export function CTA({ install }: { install: ThemedHtml }) {
  return (
    <Section className="pb-32 lg:pb-40">
      <div className={`${TEXT_COLUMN} space-y-6`}>
        <h2 className={H2}>Render your first image.</h2>
        <p className={LEAD}>
          Native on Node.js and Bun. WebAssembly on Workers and browsers. A crate for Rust.
        </p>
      </div>
      <div className={`${VISUAL_COLUMN} space-y-6`}>
        <Code html={install} />
        <div className="flex flex-wrap gap-6 text-sm">
          <Link to="/docs" className={LINK}>
            Quick start
          </Link>
          <Link to="/playground" className={LINK}>
            Playground
          </Link>
          <a
            href="https://github.com/kane50613/takumi"
            target="_blank"
            rel="noopener noreferrer"
            className={LINK}
          >
            GitHub
          </a>
        </div>
        <p className="text-sm text-muted-foreground">
          Layout by Taffy, text by Parley and Skrifa, SVG by resvg. MIT or Apache-2.0.
        </p>
      </div>
    </Section>
  );
}
