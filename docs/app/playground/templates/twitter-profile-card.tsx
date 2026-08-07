const stats = [
  { value: "125K", label: "Following" },
  { value: "8.2M", label: "Followers" },
];

export default function Profile() {
  return (
    <div tw="flex h-full w-full flex-col border-t-8 border-t-blue-500 bg-slate-900 px-16 py-12 text-white">
      <div tw="flex items-center">
        <img
          src="https://avatars.githubusercontent.com/u/1024025"
          alt=""
          tw="h-32 w-32 rounded-full border-4 border-slate-700"
        />
        <div tw="ml-8 flex flex-col">
          <span tw="text-5xl font-bold">Linus Torvalds</span>
          <span tw="mt-2 text-3xl text-slate-400">@torvalds</span>
        </div>
      </div>

      <p tw="mt-8 mb-0 max-w-[820px] text-4xl leading-tight text-slate-200">
        Wrote Linux in 1991 and Git in 2005. Still reviews patches on the mailing list.
      </p>

      <div tw="mt-auto flex text-3xl">
        {stats.map((stat) => (
          <div key={stat.label} tw="mr-12 flex text-slate-400">
            <strong tw="mr-3 font-bold text-white">{stat.value}</strong>
            {stat.label}
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
};
