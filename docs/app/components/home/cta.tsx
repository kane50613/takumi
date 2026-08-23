import { Link } from "waku";
import { ShikiHtml, type ThemedHtml } from "./shiki-html";

const linkClass = "underline underline-offset-4 decoration-border hover:decoration-primary";

export function CTA({ highlightedHtml }: { highlightedHtml: ThemedHtml }) {
  return (
    <section className="px-6 pt-32 pb-24 max-sm:pt-20 max-sm:pb-16">
      <div className="max-w-130 mx-auto text-center">
        <span
          lang="ja"
          title="takumi: artisan"
          className="inline-flex size-9 items-center justify-center bg-primary text-primary-foreground font-bold select-none mb-6"
        >
          匠
        </span>
        <h2 className="font-[540] text-[clamp(2.25rem,3.5vw,3.25rem)] leading-[1.06] tracking-tight text-balance mb-6">
          Render your first image.
        </h2>
        <ShikiHtml
          html={highlightedHtml}
          className="inline-block px-6 py-2.5 border border-dashed border-border font-mono text-sm select-all mb-6"
        />
        <div className="flex justify-center gap-6 text-sm mb-8">
          <Link to="/docs" className={linkClass}>
            Quick start
          </Link>
          <Link to="/playground" className={linkClass}>
            Playground
          </Link>
          <a
            href="https://github.com/kane50613/takumi"
            target="_blank"
            rel="noopener noreferrer"
            className={linkClass}
          >
            GitHub
          </a>
        </div>
        <div className="font-mono text-xs text-muted-foreground leading-relaxed">
          <p className="text-balance">
            Layout by taffy · text by parley &amp; skrifa · SVG by resvg
          </p>
          <p>MIT / Apache-2.0</p>
        </div>
        <nav className="mt-6 flex flex-wrap justify-center gap-x-5 gap-y-2 text-xs text-muted-foreground">
          <Link to="/about" className={linkClass}>
            About
          </Link>
          <Link to="/contact" className={linkClass}>
            Contact
          </Link>
          <Link to="/privacy" className={linkClass}>
            Privacy
          </Link>
          <a href="/llms.txt" className={linkClass}>
            llms.txt
          </a>
          <a href="/openapi.json" className={linkClass}>
            OpenAPI
          </a>
        </nav>
      </div>
    </section>
  );
}
