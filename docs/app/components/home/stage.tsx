import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { cn } from "~/lib/utils";
import { boxes, CARD } from "./card";
import { StageController } from "./stage-controller";
import { DISPLAY, GUTTER, LEAD, LINK } from "./typography";
import { UseCaseWall } from "./use-cases";

const steps = [
  { name: "Markup", detail: "JSX, HTML, and CSS become a tree of nodes." },
  { name: "Layout", detail: "Taffy places each box. Parley and Skrifa shape the text." },
  { name: "Paint", detail: "Each box becomes ordered draw steps: fills, text runs, images." },
  { name: "Pixels", detail: "The raster backend fills the canvas and encodes the file." },
];

/**
 * The homepage's opening: the headline, which scrolling breaks into the rendered card while the
 * steps below name each phase.
 */
export function Stage() {
  return (
    <section
      data-stage
      aria-labelledby="home-title"
      className="relative h-[280svh] motion-reduce:h-[180svh]"
    >
      <div
        data-stage-view
        className="sticky top-14 flex h-[calc(100svh-3.5rem)] flex-col overflow-hidden"
      >
        <StageController
          data={{ image: CARD.image, width: CARD.width, height: CARD.height, boxes }}
        />

        <div className={cn("relative min-h-0 flex-1 pt-[5svh]", GUTTER)}>
          <div className="relative size-full [container-type:size]">
            <img
              data-card
              src={CARD.image}
              alt={`The rendered card: “${CARD.headline}” in white and red on a dark ground.`}
              width={CARD.width}
              height={CARD.height}
              decoding="async"
              fetchPriority="low"
              className="absolute inset-0 m-auto h-auto w-[min(100cqw,190.5cqh)] opacity-0"
            />
          </div>
        </div>

        <ol
          data-steps
          className={cn(
            "relative grid gap-x-10 gap-y-3 py-[5svh] opacity-0 sm:grid-cols-2 lg:grid-cols-4",
            GUTTER,
          )}
        >
          {steps.map(({ name, detail }) => (
            <li
              key={name}
              className="group space-y-1 text-muted-foreground transition-colors duration-200 ease-out aria-[current=step]:text-foreground"
            >
              <span className="flex items-center gap-3 text-[1.375rem] font-[560] leading-tight">
                <span
                  aria-hidden
                  className="size-2.5 shrink-0 bg-current transition-colors duration-200 ease-out group-aria-[current=step]:bg-primary"
                />
                {name}
              </span>
              <span className="block text-lg leading-snug max-lg:hidden max-lg:group-aria-[current=step]:block">
                {detail}
              </span>
            </li>
          ))}
        </ol>

        <div className={cn("absolute inset-x-0 top-0 pt-[7svh]", GUTTER)}>
          <h1 data-headline id="home-title" className={DISPLAY}>
            One component,
            <br />
            every output.
          </h1>
          <div data-hero className="mt-[4svh] space-y-[5svh]">
            <div className="grid gap-x-12 gap-y-6 lg:grid-cols-[minmax(0,36rem)_auto] lg:items-end">
              <p className={LEAD}>
                Takumi renders JSX, HTML, and CSS to images, animations, SVG, PDF, or paint data. No
                headless browser.
              </p>
              <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
                <Button
                  asChild
                  size="lg"
                  className="h-12 px-7 text-base font-semibold active:scale-[0.97]"
                >
                  <Link to="/docs">Get started</Link>
                </Button>
                <Link to="/playground" className={cn(LINK, "text-lg")}>
                  Open playground
                </Link>
              </div>
            </div>
            <UseCaseWall />
          </div>
        </div>
      </div>
    </section>
  );
}
