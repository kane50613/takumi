const INDEX: { label: string; mono: boolean; items: string[] }[] = [
  {
    label: "Layout",
    mono: true,
    items: ["display: grid", "float", "position: absolute", "calc()", "z-index"],
  },
  { label: "Selectors", mono: true, items: [":is()", ":where()", "::before", "::after"] },
  {
    label: "Paint",
    mono: true,
    items: [
      "backdrop-filter",
      "mix-blend-mode",
      "conic-gradient()",
      "clip-path",
      "mask",
      "background-clip: text",
    ],
  },
  {
    label: "Text",
    mono: false,
    items: ["WOFF2 fonts", "emoji", "RTL scripts", "multi-span inline"],
  },
  { label: "Motion", mono: true, items: ["@keyframes", "animation", "Tailwind animate-*"] },
];

export function Features() {
  return (
    <section className="px-6 pt-20 pb-24 max-sm:py-14">
      <div className="max-w-275 mx-auto">
        <h2 className="font-[540] text-[clamp(2.25rem,4vw,3.5rem)] leading-[1.06] tracking-tight mb-4">
          The CSS you write.
        </h2>
        <p className="text-muted-foreground leading-relaxed max-w-150 mb-10">
          Not a tiny OG subset. Put the property back.
        </p>

        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-x-8 gap-y-10 max-w-250">
          {INDEX.map(({ label, mono, items }) => (
            <div key={label} className="border-t border-border pt-4">
              <h3 className="font-mono text-xs uppercase tracking-[0.18em] text-foreground mb-4">
                {label}
              </h3>
              <ul className="space-y-2.5 text-sm text-muted-foreground">
                {items.map((item) => (
                  <li key={item} className={mono ? "font-mono text-[0.8125rem]" : ""}>
                    {item}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <p className="mt-12 text-muted-foreground leading-relaxed max-w-150">
          Native on Node. WASM on Workers and browsers. Crate for Rust.
        </p>
        <p className="mt-4 text-muted-foreground leading-relaxed max-w-150">
          People ship open graph cards, invoices, receipts, tickets, certificates, and paged PDF
          reports with it. One template renders to PNG, WebP, AVIF, JPEG, SVG, or PDF. No headless
          browser to install, keep warm, or pay for.
        </p>
        <p className="mt-2 font-mono text-xs text-muted-foreground">
          macOS · Linux · Windows · x64 · ARM64
        </p>
      </div>
    </section>
  );
}
