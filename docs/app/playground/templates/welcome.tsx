const swatches = ["bg-coral", "bg-honey", "bg-mint", "bg-sky"];

export default function Welcome() {
  return (
    <div tw="flex h-full w-full flex-col justify-end bg-ink p-20">
      <div tw="flex">
        {swatches.map((swatch, index) => (
          <div
            key={swatch}
            tw={`h-6 w-${24 - index * 6} rounded-full ${swatch} ${index ? "ml-3" : ""}`}
          />
        ))}
      </div>
      <h1 tw="m-0 mt-10 text-8xl font-bold leading-none tracking-tighter text-paper">
        Edit the code.
      </h1>
      <h1 tw="m-0 mt-2 text-8xl font-bold leading-none tracking-tighter text-coral">Press Run.</h1>
      <p tw="mt-10 mb-0 text-3xl leading-snug text-faded">
        The exported options decide what comes out: an image, an animation, or a PDF. The colours
        here come from <span tw="text-honey">options.variables</span> — rename a token and every
        class reading it follows.
      </p>
    </div>
  );
}

export const options: PlaygroundOptions = {
  width: 1200,
  height: 630,
  format: "png",
  variables: {
    "--color-ink": "#16130f",
    "--color-paper": "#fdfaf4",
    "--color-faded": "#a8a29a",
    "--color-coral": "#ff4d4d",
    "--color-honey": "#fbbf24",
    "--color-mint": "#34d399",
    "--color-sky": "#38bdf8",
  },
};
