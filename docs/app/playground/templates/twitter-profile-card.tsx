const stats = [
  { value: "125K", label: "Following" },
  { value: "8.2M", label: "Followers" },
];

export default function Profile() {
  return (
    <div tw="flex h-full w-full flex-col border-t-8 border-t-accent bg-surface px-16 py-12 text-bright">
      <div tw="flex items-center">
        <img
          src="https://avatars.githubusercontent.com/u/1024025"
          alt=""
          tw="h-32 w-32 rounded-full border-4 border-accent/40"
        />
        <div tw="ml-8 flex flex-col">
          <span tw="text-5xl font-bold">Linus Torvalds</span>
          <span tw="mt-2 text-3xl text-accent">@torvalds</span>
        </div>
      </div>

      <p tw="mt-8 mb-0 max-w-[820px] text-4xl leading-tight text-body">
        Wrote Linux in 1991 and Git in 2005. Still reviews patches on the mailing list.
      </p>

      <div tw="mt-auto flex text-3xl">
        {stats.map((stat) => (
          <div key={stat.label} tw="mr-12 flex text-dim">
            <strong tw="mr-3 font-bold text-bright">{stat.value}</strong>
            {stat.label}
          </div>
        ))}
      </div>
    </div>
  );
}

// One accent token re-skins the border, the handle and the avatar ring together.
export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
  cssVariables: {
    "--color-accent": "#2dd4bf",
    "--color-surface": "#0f172b",
    "--color-bright": "#f8fafc",
    "--color-body": "#e2e8f0",
    "--color-dim": "#90a1b9",
  },
};
