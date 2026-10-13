import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { cn } from "~/lib/utils";
import { boxes, CARD, kilobytes, paintSteps, source } from "./card";
import { StageController } from "./stage-controller";
import { DISPLAY, GUTTER, LEAD, LINK } from "./typography";

const steps = [
  { name: "JSX", detail: "A component with Tailwind classes. No browser, no DOM." },
  { name: "Layout", detail: `Taffy places all ${boxes.length} boxes with flexbox.` },
  { name: "Paint", detail: `The boxes become ${paintSteps.length} draw steps.` },
  {
    name: "Pixels",
    detail: `${CARD.width * 2} × ${CARD.height * 2}, ${kilobytes(CARD.image)} as WebP.`,
  },
];

/**
 * The homepage's opening: the hero over the card's own source, which scrolling turns into the
 * rendered card while the steps below name each phase.
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
          data={{ source, image: CARD.image, width: CARD.width, height: CARD.height, boxes }}
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
          <div
            data-hero
            className="grid gap-x-12 gap-y-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,30rem)] lg:items-end"
          >
            <h1 id="home-title" className={DISPLAY}>
              JSX to images.
              <br />
              No browser.
            </h1>
            <div className="space-y-5 lg:pb-2">
              <p className={LEAD}>
                Takumi renders OG images, social cards, and PDFs from JSX and CSS, in Rust.
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
                  Playground
                </Link>
                <code className="font-mono text-base select-all">bun i takumi-js</code>
              </div>
            </div>
          </div>

          <pre
            data-source
            aria-label="home-demo-card.tsx"
            className="mt-[6svh] overflow-visible font-mono text-[0.9375rem] leading-[1.7] lg:text-base"
          >
            <code>
              {source.map((line, index) => (
                <span key={index} className="block whitespace-pre">
                  {line || " "}
                </span>
              ))}
            </code>
          </pre>
        </div>
      </div>
    </section>
  );
}
