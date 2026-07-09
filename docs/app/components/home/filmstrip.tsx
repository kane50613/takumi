const OUTPUT_BASE = "/images/twitter-images";

const TIMESTAMPS = [0, 125, 250, 375, 500, 625, 750, 875];

// Stylesheet of example/twitter-images/components/home-filmstrip.tsx; keep in sync.
const MORPH_KEYFRAMES = `@keyframes morph {
  from { border-radius: 12%; transform: rotate(0deg) scale(1); }
  50%  { border-radius: 50%; transform: rotate(90deg) scale(0.72); }
  to   { border-radius: 12%; transform: rotate(180deg) scale(1); }
}`;

export function Filmstrip() {
  return (
    <section className="bg-[#16130f] text-[#f5f1ea] px-6 py-24 max-sm:py-14">
      <div className="max-w-275 mx-auto">
        <div className="flex items-end justify-between gap-12 mb-10 max-lg:flex-col max-lg:items-start max-lg:gap-8">
          <div>
            <h2 className="font-[540] text-[clamp(2.25rem,4vw,3.5rem)] leading-[1.06] tracking-tight mb-4">
              One tree, sampled across time.
            </h2>
            <p className="text-[#a8a29a] leading-relaxed max-w-150">
              Pass a timestamp. PNG is t&nbsp;=&nbsp;0. Animated WebP is the same tree over t.
            </p>
          </div>
          <pre className="shrink-0 font-mono text-xs leading-relaxed text-[#a8a29a] whitespace-pre max-lg:overflow-x-auto max-lg:max-w-full">
            {MORPH_KEYFRAMES}
          </pre>
        </div>

        <div className="overflow-x-auto">
          <div className="grid grid-cols-8 gap-px min-w-160">
            {TIMESTAMPS.map((t, index) => (
              <figure key={t}>
                <img
                  src={`${OUTPUT_BASE}/home-filmstrip-${index}.webp`}
                  alt={`Frame sampled at t=${t}ms`}
                  width={480}
                  height={480}
                  loading="lazy"
                  className="w-full h-auto"
                />
                <figcaption className="mt-2.5 font-mono text-xs max-sm:text-sm text-[#a8a29a]">
                  t={t}
                </figcaption>
              </figure>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
