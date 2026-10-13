import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { cn } from "~/lib/utils";
import { CARD, node } from "./card";
import { LiveCard } from "./live-card";
import { BODY, GUTTER, H2, LINK, VALUE } from "./typography";

/** The page's end: the card, rendered again in the reader's browser as they edit it. */
export function Live() {
  return (
    <section aria-labelledby="live-title" className={cn("py-[14svh]", GUTTER)}>
      <LiveCard
        node={node}
        headline={CARD.headline}
        image={CARD.image}
        width={CARD.width}
        height={CARD.height}
        intro={
          <div className="space-y-4">
            <h2 id="live-title" className={H2}>
              Try it.
            </h2>
            <p className={BODY}>
              Edit the headline. Takumi’s WebAssembly build renders the card again in your browser.
            </p>
          </div>
        }
        outro={
          <div className="space-y-6 pt-4">
            <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
              <Button
                asChild
                size="lg"
                className="h-12 px-7 text-base font-semibold active:scale-[0.97]"
              >
                <Link to="/docs">Get started</Link>
              </Button>
              <Link to="/playground" className={cn(LINK, "text-lg")}>
                Open the full playground
              </Link>
              <a
                href="https://github.com/kane50613/takumi"
                target="_blank"
                rel="noopener noreferrer"
                className={cn(LINK, "text-lg")}
              >
                View on GitHub
              </a>
            </div>
            <p className={VALUE}>Layout by Taffy. Text by Parley and Skrifa. MIT or Apache-2.0.</p>
          </div>
        }
      />
    </section>
  );
}
