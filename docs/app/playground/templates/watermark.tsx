const clauses = [
  {
    title: "1. Confidential information",
    body: "Each party may disclose material it treats as confidential. The receiving party protects it with the same care it applies to its own material of like importance, and never with less than reasonable care.",
  },
  {
    title: "2. Permitted use",
    body: "The receiving party uses the material only to evaluate the transaction described in Schedule A. Any other use, including training a model or benchmarking against a competing product, needs written permission.",
  },
  {
    title: "3. Named recipients",
    body: "Disclosure inside the receiving party is limited to employees and advisers who need the material for the permitted use and who are bound by obligations at least as strict as these.",
  },
  {
    title: "4. Return and destruction",
    body: "On written request the receiving party returns or destroys the material within thirty days, and confirms in writing that it did. Backups made in the ordinary course may be kept until they expire.",
  },
  {
    title: "5. Term",
    body: "These obligations run for three years from the last disclosure. Material that qualifies as a trade secret stays protected for as long as it qualifies.",
  },
  {
    title: "6. No licence",
    body: "Nothing here transfers ownership or grants a licence under any patent, copyright, or trademark. The material is provided as is, with no warranty of accuracy or completeness.",
  },
  {
    title: "7. Governing law",
    body: "This agreement is governed by the laws of Taiwan. The parties submit to the exclusive jurisdiction of the Taipei District Court.",
  },
  {
    title: "8. Residual knowledge",
    body: "Nothing prevents a person from using general knowledge, skills, and experience retained in unaided memory. This clause does not license the deliberate memorisation of the material.",
  },
  {
    title: "9. Notices",
    body: "Notices are given in writing to the addresses in Schedule A, by hand, by courier, or by email with confirmed receipt. A notice takes effect when it arrives.",
  },
  {
    title: "10. Assignment",
    body: "Neither party assigns this agreement without the other's written consent, except to a successor of substantially the whole of its business, who is bound by the same obligations.",
  },
  {
    title: "11. Remedies",
    body: "The parties agree that damages alone may not remedy a breach, and that the disclosing party may seek injunctive relief without posting a bond.",
  },
  {
    title: "12. Entire agreement",
    body: "This document, with Schedule A, is the whole agreement about its subject. It replaces earlier drafts, and it changes only in writing signed by both parties.",
  },
];

export default function Watermark() {
  // No background on the root: a negative z-index paints under the content, and
  // a root background would cover it.
  return (
    <div tw="flex w-full flex-col text-[#111827]">
      {/*
        A fixed box the page holds repeats on every page, laid out against the
        page area. The negative z-index keeps it under the text, and the rotate
        on the box itself does not change what it is positioned against.
      */}
      <div
        tw="fixed inset-0 flex items-center justify-center"
        style={{ zIndex: -1, transform: "rotate(-30deg)" }}
      >
        <span
          tw="w-full text-center text-[8vw] font-bold tracking-[0.14em]"
          style={{ color: "rgba(17,24,39,0.14)" }}
        >
          CONFIDENTIAL
        </span>
      </div>

      <h1 tw="mt-0 mb-1 text-2xl font-bold">Mutual non-disclosure agreement</h1>
      <span tw="text-xs text-[#6b7280]">
        Between Kiln Studio Ltd. and Aomori Systems K.K. · 2026-08-14
      </span>

      {clauses.map((clause) => (
        <div key={clause.title} tw="mt-6 flex flex-col break-inside-avoid">
          <h2 tw="m-0 text-base font-semibold">{clause.title}</h2>
          <p tw="mt-2 mb-0 text-sm leading-6 text-[#374151]">{clause.body}</p>
        </div>
      ))}

      <div tw="mt-12 flex justify-between">
        <div tw="flex w-[45%] flex-col border-t border-[#9ca3af] pt-2 text-xs">
          <span tw="font-semibold">Kiln Studio Ltd.</span>
          <span tw="text-[#6b7280]">Name, title, date</span>
        </div>
        <div tw="flex w-[45%] flex-col border-t border-[#9ca3af] pt-2 text-xs">
          <span tw="font-semibold">Aomori Systems K.K.</span>
          <span tw="text-[#6b7280]">Name, title, date</span>
        </div>
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  pdf: {
    size: "a4",
    margin: { top: 64, right: 56, bottom: 64, left: 56 },
    footer: (
      <div tw="flex w-full justify-between px-14 pb-5 text-[10px] text-[#9ca3af]">
        <span>Confidential · draft for review</span>
        <span>
          <span className="pageNumber" /> / <span className="totalPages" />
        </span>
      </div>
    ),
    outline: true,
    metadata: { title: "Mutual non-disclosure agreement", creationDate: "2026-08-14" },
  },
};
