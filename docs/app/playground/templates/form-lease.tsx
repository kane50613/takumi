const occupants = ["", "", ""];

const blank = "h-[26px] border-0 border-b border-[#9aa1ab] bg-transparent px-1.5 text-xs";
const boxed = "h-8 w-full rounded border border-[#ccd2da] bg-white px-3 text-xs";
const tick = "mr-2.5 h-4 w-4 border border-[#8d95a0] bg-white";
const heading = "text-[11px] font-semibold uppercase tracking-wider text-[#6b7280]";

export default function Lease() {
  return (
    <div tw="flex w-full flex-col text-[#1f2328]">
      <div tw="flex flex-col">
        <h1 tw="m-0 text-xl font-semibold">Simple Rental Agreement</h1>
        <span tw="mt-1 text-[11px] text-[#6b7280]">
          Fill this in your PDF reader. Nothing here needs printing.
        </span>
      </div>

      <div tw="mt-7 flex flex-col text-xs">
        <div tw="flex items-center">
          <span tw="mr-2">This agreement is between</span>
          <input
            name="party.landlord"
            defaultValue="Elisa Beckett"
            aria-label="Landlord"
            tw={`w-[180px] ${blank}`}
          />
          <span tw="mx-2">(landlord) and</span>
          <input
            name="party.tenant"
            defaultValue="John Smith"
            aria-label="Tenant"
            tw={`w-[180px] ${blank}`}
          />
          <span tw="ml-2">(tenant).</span>
        </div>

        <div tw="mt-5 flex items-center">
          <span tw="mr-2">Takes effect on</span>
          <input
            name="term.start"
            defaultValue="2026-10-01"
            aria-label="Start date"
            tw={`w-[110px] ${blank}`}
          />
          <span tw="mx-2">and runs for</span>
          <select
            name="term.length"
            defaultValue="12"
            aria-label="Term length"
            tw={`w-[110px] ${blank}`}
          >
            <option value="6">6 months</option>
            <option value="12">12 months</option>
            <option value="24">24 months</option>
          </select>
        </div>

        <div tw="mt-5 flex items-center">
          <span tw="mr-2">Rent of $</span>
          <input
            name="rent.amount"
            defaultValue="1450"
            maxLength={7}
            aria-label="Monthly rent"
            tw={`w-[80px] ${blank} text-right`}
          />
          <span tw="mx-2">is due on day</span>
          <input
            name="rent.dueDay"
            defaultValue="1"
            maxLength={2}
            aria-label="Due day"
            tw={`w-[40px] ${blank} text-center`}
          />
          <span tw="ml-2">of each month.</span>
        </div>
      </div>

      <span tw={`mt-9 ${heading}`}>Additional occupants</span>
      <div tw="mt-2 flex flex-col">
        {occupants.map((_, index) => (
          <input
            key={index}
            name={`occupant.${index + 1}`}
            aria-label={`Occupant ${index + 1}`}
            tw={`mt-2.5 ${boxed}`}
          />
        ))}
      </div>

      <span tw={`mt-9 ${heading}`}>Utilities included in the rent</span>
      <div tw="mt-2 flex">
        {["water", "power", "gas", "internet"].map((utility) => (
          <div key={utility} tw="mr-6 flex items-center">
            <input
              type="checkbox"
              name={`utility.${utility}`}
              defaultChecked={utility === "water"}
              aria-label={utility}
              tw={`${tick} rounded-[2px]`}
            />
            <span tw="text-xs capitalize">{utility}</span>
          </div>
        ))}
      </div>

      <span tw={`mt-9 ${heading}`}>Deposit returned after the lease expires</span>
      <div tw="mt-2 flex">
        {["yes", "no"].map((answer) => (
          <div key={answer} tw="mr-6 flex items-center">
            <input
              type="radio"
              name="deposit"
              value={answer}
              defaultChecked={answer === "yes"}
              aria-label={answer}
              tw={`${tick} rounded-full`}
            />
            <span tw="text-xs capitalize">{answer}</span>
          </div>
        ))}
      </div>

      <span tw={`mt-9 ${heading}`}>Notes</span>
      <textarea
        name="notes"
        aria-label="Notes"
        tw="mt-3 h-[84px] w-full rounded border border-[#ccd2da] bg-white p-3 text-xs"
      >
        The tenant may repaint with written consent.
      </textarea>

      <div tw="mt-12 flex items-end justify-between">
        <div tw="flex flex-col">
          <input
            name="signature.tenant"
            required
            aria-label="Signature of tenant"
            tw={`w-[230px] ${blank} h-10 text-base`}
          />
          <span tw="mt-1 text-[10px] text-[#6b7280]">Signature of tenant</span>
        </div>
        <div tw="flex flex-col">
          <input
            name="signature.date"
            aria-label="Date signed"
            tw={`w-[120px] ${blank} h-10 text-base`}
          />
          <span tw="mt-1 text-[10px] text-[#6b7280]">Date</span>
        </div>
      </div>

      <span tw="mt-6 text-[10px] text-[#9ca3af]">
        Reference AGR-2026-0413 · generated by takumi
      </span>
    </div>
  );
}

export const options: PlaygroundOptions = {
  pdf: {
    size: "a4",
    margin: 56,
    form: true,
    metadata: {
      title: "Simple Rental Agreement",
      authors: ["Kiln Werkstatt"],
      creationDate: "2026-03-01",
    },
    lang: "en",
  },
};
