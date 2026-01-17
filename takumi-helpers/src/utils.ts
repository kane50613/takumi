const defaultTimeout = 5000;

export async function fetchResources(urls: string[], timeout = defaultTimeout) {
  const signal = AbortSignal.timeout(timeout);
  const promises = urls.map(
    async (url) =>
      [
        url,
        await fetch(url, {
          signal,
        }).then((r) => r.arrayBuffer()),
      ] as const,
  );

  const resources = await Promise.all(promises);

  return new Map(resources);
}
