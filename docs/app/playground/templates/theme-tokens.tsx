const tiers = [
  { name: "Base", price: "$0", tw: "bg-surface text-ink" },
  { name: "Pro", price: "$29", tw: "bg-brand text-paper" },
  { name: "Team", price: "$99", tw: "bg-accent text-ink" },
];

export default function Pricing() {
  return (
    <div tw="flex h-full w-full flex-col justify-center bg-paper p-gutter">
      <h1 tw="m-0 text-display font-bold tracking-tight text-ink">Pricing</h1>
      <div tw="mt-8 flex">
        {tiers.map((tier, index) => (
          <div
            key={tier.name}
            tw={`flex flex-1 flex-col rounded-card p-8 ${tier.tw} ${index ? "ml-6" : ""}`}
          >
            <span tw="text-3xl opacity-80">{tier.name}</span>
            <span tw="mt-3 text-6xl font-black">{tier.price}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
  css: `
    @theme {
      --color-paper: #f6f5f2;
      --color-surface: #e6e3dc;
      --color-ink: #1c1a17;
      --color-brand: #6d28d9;
      --color-accent: #fcd34d;
      --spacing-gutter: 4rem;
      --radius-card: 1.5rem;
      --text-display: 5rem;
    }
  `,
};
