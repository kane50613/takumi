import { cn } from "~/lib/utils";
import { GUTTER, H2, VALUE } from "./typography";

const groups = [
  {
    id: "outputs-title",
    title: "One tree, five outputs.",
    items: [
      { name: "Images", detail: "PNG, WebP, and JPEG at any size." },
      { name: "Animations", detail: "CSS keyframes become animated WebP, APNG, or GIF." },
      { name: "SVG", detail: "Vector output from the same layout." },
      { name: "PDF", detail: "Paged documents with selectable text and repeating headers." },
      { name: "Paint data", detail: "Draw steps for Canvas, slides, or your own renderer." },
    ],
  },
  {
    id: "runtimes-title",
    title: "Same render, every runtime.",
    items: [
      {
        name: "Node.js and Bun",
        detail: "Native binding, prebuilt for macOS, Linux, and Windows.",
      },
      { name: "Workers and browsers", detail: "WebAssembly build. Cloudflare Workers included." },
      { name: "Rust", detail: "Embed the takumi crate." },
    ],
  },
];

/** What the tree renders to, and where the renderer runs. */
export function Outputs() {
  return (
    <div className={cn("space-y-[14svh] py-[14svh]", GUTTER)}>
      {groups.map(({ id, title, items }) => (
        <section key={id} aria-labelledby={id} className="space-y-10">
          <h2 id={id} className={H2}>
            {title}
          </h2>
          <dl className="grid gap-x-10 gap-y-8 sm:grid-cols-2 lg:grid-cols-5">
            {items.map(({ name, detail }) => (
              <div key={name} className="space-y-1">
                <dt className="text-[1.375rem] font-[560] leading-tight">{name}</dt>
                <dd className={VALUE}>{detail}</dd>
              </div>
            ))}
          </dl>
        </section>
      ))}
    </div>
  );
}
