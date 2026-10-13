import { SvgIds, sceneSteps } from "~/components/mdx/paint/paint-svg";
import { boxes, CARD, layers, scene } from "./card";

const STATES = [
  { id: "jsx", label: "JSX" },
  { id: "layout", label: "Layout" },
  { id: "paint", label: "Paint" },
  { id: "pixels", label: "Pixels" },
  { id: "motion", label: "Animation" },
] as const;

export type StateId = (typeof STATES)[number]["id"];

const ids = new SvgIds("stage");

function LayoutBoxes() {
  return (
    <svg
      viewBox={`0 0 ${CARD.width} ${CARD.height}`}
      className="story-boxes absolute inset-0 size-full overflow-visible"
      aria-hidden
    >
      {boxes.map(({ name, depth, bounds }, index) => {
        const leaf = !boxes.some(
          (other) =>
            other.depth > depth &&
            other.bounds.x >= bounds.x &&
            other.bounds.y >= bounds.y &&
            other.bounds.x + other.bounds.width <= bounds.x + bounds.width &&
            other.bounds.y + other.bounds.height <= bounds.y + bounds.height,
        );

        return (
          <g key={index}>
            <rect
              {...bounds}
              fill="none"
              stroke="white"
              strokeOpacity={leaf ? 0.85 : 0.35}
              strokeWidth={2}
              strokeDasharray={leaf ? undefined : "8 6"}
              vectorEffect="non-scaling-stroke"
            />
            {leaf && (
              <text
                x={bounds.x}
                y={bounds.y + bounds.height + 26}
                fill="white"
                fillOpacity={0.85}
                fontSize={20}
                fontFamily="var(--font-mono)"
              >
                {name} {Math.round(bounds.width)}×{Math.round(bounds.height)}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}

/**
 * Styles that depend on a layer's index: its place in the exploded stack, and the highlight
 * when a reader hovers its row in the step list.
 */
export function layerStyles() {
  const middle = (layers.length - 1) / 2;

  return layers
    .map(
      (_, index) =>
        `[data-layer="${index}"]{--offset:${index - middle}}` +
        `@media (hover:hover){.story:has([data-row="${index}"]:hover) .story-layer:not([data-layer="${index}"]){opacity:.15}` +
        `.story:has([data-row="${index}"]:hover) [data-layer="${index}"] .story-outline{border-color:var(--primary)}}`,
    )
    .join("");
}

/** The card the whole story follows; `data-state` on the figure picks how it is shown. */
export function Stage() {
  return (
    <figure data-stage data-state="jsx" className="story-stage w-full">
      <div className="story-view">
        <div
          className="story-card relative"
          style={{ aspectRatio: `${CARD.width} / ${CARD.height}` }}
        >
          <img
            src={CARD.image}
            srcSet={`${CARD.imageSmall} ${CARD.width}w, ${CARD.image} ${CARD.width * 2}w`}
            sizes="(min-width: 64rem) 600px, 100vw"
            width={CARD.width}
            height={CARD.height}
            alt="A dark card that reads: This card is the code beside it."
            fetchPriority="high"
            className="story-image absolute inset-0 size-full rounded-[1.4cqw]"
          />
          <div className="story-layers absolute inset-0">
            {layers.map((layer, index) => {
              const step = scene.steps[layer.step];

              return (
                <div key={layer.step} data-layer={index} className="story-layer absolute inset-0">
                  <svg
                    viewBox={`0 0 ${CARD.width} ${CARD.height}`}
                    className="absolute inset-0 size-full overflow-visible"
                    aria-hidden
                  >
                    {step && sceneSteps({ ...scene, steps: [step] }, 1, ids)}
                  </svg>
                  <div className="story-outline absolute inset-0 rounded-[1.4cqw] border" />
                </div>
              );
            })}
          </div>
          <LayoutBoxes />
          <img
            data-src={CARD.motion}
            alt=""
            width={CARD.width}
            height={CARD.height}
            className="story-motion absolute inset-0 size-full rounded-[1.4cqw]"
          />
          <canvas className="story-pixels absolute top-[-24%] left-[-8%] h-[148%] w-[116%]" />
        </div>
      </div>
      <nav
        aria-label="Stages"
        className="story-stepper relative z-10 mt-6 flex flex-wrap gap-x-5 gap-y-2 text-sm"
      >
        {STATES.map(({ id, label }) => (
          <a key={id} href={`#${id}`} data-goto={id}>
            {label}
          </a>
        ))}
      </nav>
    </figure>
  );
}
