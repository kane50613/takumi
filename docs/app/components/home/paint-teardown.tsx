import { Link } from "waku";
import { type Drawable, paint } from "takumi-paint";
import { paintScene } from "~/components/mdx/paint/scene";
import { describeDrawable, describePaint, round } from "~/playground/inspect-paint";
import DemoCard, {
  height,
  width,
} from "../../../../example/twitter-images/components/home-demo-card";
import { type Layer, PaintStack } from "./paint-stack";
import { ShikiHtml, type ThemedHtml } from "./shiki-html";

const tree = await paint(<DemoCard />, { width, height });
const scene = paintScene(tree);

function describeLayer(drawable: Drawable): Pick<Layer, "label" | "value" | "swatch"> {
  switch (drawable.type) {
    case "glyphs": {
      const { font, fontSize } = drawable.run;
      const { value, swatch } = describePaint(drawable.paint);

      return {
        label: drawable.role,
        value: `${font.family ?? "unnamed"} ${font.weight} · ${round(fontSize)}px · ${value}`,
        swatch,
      };
    }
    case "masked": {
      const [content] = drawable.content;
      const [mask] = drawable.mask;

      if (content?.type === "fill" && mask) {
        const { value, swatch } = describePaint(content.paint);

        return {
          label: drawable.role,
          value: `${value} · mask: ${mask.type === "fill" ? mask.shape.type : mask.type}`,
          swatch,
        };
      }
      break;
    }
  }

  return describeDrawable(drawable);
}

let sceneStep = -1;

const layers = tree.steps.flatMap((step, order): Layer[] => {
  // paintScene drops draws with nothing to draw, so the scene's step indices skip them too.
  if (step.type === "draw" && step.drawables.length === 0) {
    return [];
  }
  sceneStep++;

  const [first] = step.type === "draw" ? step.drawables : [];
  const node = scene.nodes[scene.steps[sceneStep]?.node ?? -1];

  return first
    ? [{ step: sceneStep, order, name: node?.name ?? step.node.type, ...describeLayer(first) }]
    : [];
});

const USES = [
  "Write slides where text stays editable, such as PPTX.",
  "Draw on Canvas, Skia, or a native drawing API.",
  "Read where layout put each box and line of text.",
];

const linkClass = "underline underline-offset-4 decoration-white/25 hover:decoration-primary";

export function PaintTeardown({ highlightedHtml }: { highlightedHtml: ThemedHtml }) {
  return (
    <section className="dark bg-[#0d0c0b] text-[#f5f1ea]">
      <PaintStack scene={scene} layers={layers}>
        <h2 className="font-[540] text-[clamp(2.25rem,4vw,3.5rem)] leading-[1.06] tracking-tight mb-4">
          Take the render apart.
        </h2>
        <p className="text-[#a8a29a] leading-relaxed max-w-140">
          <code className="font-mono text-[0.9em] text-[#f5f1ea]">takumi-paint</code> lays out the
          same JSX and returns what to draw instead of pixels. Here is the card above, one draw step
          per layer.
        </p>
      </PaintStack>

      <div className="px-6 pb-28 max-sm:pb-16">
        <div className="max-w-275 mx-auto grid lg:grid-cols-[minmax(0,1.15fr)_minmax(0,1fr)] gap-12 items-start">
          <ShikiHtml
            html={highlightedHtml}
            className="rounded-xl border border-white/10 bg-white/[0.03] p-5 font-mono text-[0.72rem] sm:text-[0.8rem] leading-relaxed overflow-x-auto"
          />
          <div>
            <ul className="space-y-3 text-[#d9d3ca] leading-relaxed mb-8">
              {USES.map((use) => (
                <li key={use} className="flex gap-3">
                  <span aria-hidden className="mt-2.5 size-1.5 shrink-0 bg-primary" />
                  {use}
                </li>
              ))}
            </ul>
            <p className="font-mono text-sm text-[#a8a29a] mb-6 select-all">npm i takumi-paint</p>
            <div className="flex gap-6 text-sm">
              <Link to="/docs/paint" className={linkClass}>
                Read the paint docs
              </Link>
              <Link to="/docs/paint/drawing" className={linkClass}>
                Replay the steps
              </Link>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
