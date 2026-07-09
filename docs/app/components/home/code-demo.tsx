import { ShikiHtml, type ThemedHtml } from "./shiki-html";

const DEMO_CARD_URL = "/images/twitter-images/home-demo-card@2x.webp";

export function CodeDemo({ highlightedHtml }: { highlightedHtml: ThemedHtml }) {
  return (
    <section className="px-6 py-24 max-sm:py-14">
      <div className="max-w-275 mx-auto">
        <h2 className="font-[540] text-[clamp(2.25rem,4vw,3.5rem)] leading-[1.06] tracking-tight mb-4">
          The code is the design file.
        </h2>
        <p className="text-muted-foreground leading-relaxed max-w-150 mb-10">
          Drop-in for next/og. The card is this source file, rendered.
        </p>

        <div className="grid xl:grid-cols-2 gap-10 items-start">
          <ShikiHtml
            html={highlightedHtml}
            className="bg-muted/30 p-5 font-mono text-[0.78rem] leading-relaxed overflow-x-auto"
          />
          <div className="max-w-160 xl:max-w-none">
            <img
              src={DEMO_CARD_URL}
              alt="The card rendered from the source on the left"
              width={1200}
              height={630}
              className="w-full h-auto border border-border"
            />
            <p className="mt-3 font-mono text-xs text-muted-foreground">
              home-demo-card@2x.webp · 52 KB · rendered by bun example/twitter-images
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
