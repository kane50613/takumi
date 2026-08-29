const sections = [
  {
    title: "Summary",
    body: "Every heading below becomes a bookmark and a tagged structure element, so a screen reader walks the document in reading order.",
  },
  {
    title: "Method",
    body: "Each run renders the same four-page document ten times on a warm renderer, and the middle eight runs are kept.",
  },
  {
    title: "Results",
    body: "Median render time fell from 41 ms to 28 ms. Per-document font subsetting took a four-page invoice from 78 KB to 62 KB.",
  },
  {
    title: "Next steps",
    body: "Cold starts still dominate the first request, so a shared renderer is worth keeping alive between jobs.",
  },
];

export default function Accessible() {
  return (
    <div tw="flex w-full flex-col text-[#1f2430]">
      <h1 tw="m-0 text-3xl font-semibold">Quarterly notes</h1>

      {sections.map((section) => (
        <div key={section.title} tw="mt-8 flex flex-col">
          <h2 tw="m-0 border-l-4 border-[#4338ca] pl-3 text-lg font-semibold">{section.title}</h2>
          <p tw="mt-2 mb-0 text-sm leading-6 text-[#374151]">{section.body}</p>
        </div>
      ))}
    </div>
  );
}

export const options: PlaygroundOptions = {
  pdf: {
    size: "a4",
    margin: 56,
    pdfa: "4",
    tagged: "ua2",
    outline: true,
    lang: "en",
    metadata: { title: "Quarterly notes", creationDate: "2026-04-02" },
  },
};
