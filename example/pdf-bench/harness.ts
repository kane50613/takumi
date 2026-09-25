const WARM_RUNS = 20;

/** Prints the time from `start` through the first render, the median warm render, and the output size. */
export async function benchmark(
  engine: string,
  start: number,
  render: () => Promise<Uint8Array>,
  outFile: string,
) {
  const first = await render();
  const coldMs = performance.now() - start;
  const times: number[] = [];

  for (let i = 0; i < WARM_RUNS; i++) {
    const runStart = performance.now();

    await render();
    times.push(performance.now() - runStart);
  }

  const [lower = 0, upper = 0] = times.sort((a, b) => a - b).slice(WARM_RUNS / 2 - 1);

  await Bun.write(outFile, first);
  console.log(
    JSON.stringify({
      engine,
      coldMs: Math.round(coldMs),
      warmMedianMs: Math.round((lower + upper) / 2),
      bytes: first.byteLength,
    }),
  );
}
