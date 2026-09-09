function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <div tw="mt-7 flex items-start">
      <div tw="flex w-[165px] flex-col pr-6">
        <span tw="text-[11px] font-semibold">{label}</span>
        <span tw="mt-1 text-[10px] leading-relaxed text-[#8a9099]">{hint}</span>
      </div>
      <div tw="flex flex-1 items-start">{children}</div>
    </div>
  );
}

const field = "h-8 rounded border border-[#ccd2da] bg-white px-3 text-xs";
const box = "mr-2.5 h-4 w-4 border border-[#8d95a0] bg-white";

export default function FormControls() {
  return (
    <div tw="flex w-full flex-col text-[#1f2328]">
      <h1 tw="m-0 text-lg font-semibold">Every fillable control</h1>
      <span tw="mt-1 text-[11px] text-[#8a9099]">
        Open the output in a PDF reader and type into it. Each row names the PDF field it becomes.
      </span>

      <div tw="mt-6 flex flex-col">
        <h2 tw="m-0 mb-1 text-sm font-semibold">Text fields</h2>
        <Row label="Text" hint='name="plain", starts with a value'>
          <input name="plain" defaultValue="Editable" tw={`w-[220px] ${field}`} />
        </Row>

        <Row label="Dotted name" hint="contact.email nests email under contact">
          <input
            name="contact.email"
            defaultValue="hi@example.com"
            aria-label="Email"
            tw={`w-[220px] ${field}`}
          />
        </Row>

        <Row label="maxlength" hint="The reader stops at 5 characters">
          <input name="code" maxLength={5} defaultValue="AB12" tw={`w-[90px] ${field}`} />
        </Row>

        <Row label="Password" hint="Masked in the reader, plain in the bytes">
          <input name="secret" type="password" defaultValue="hunter2" tw={`w-[140px] ${field}`} />
        </Row>

        <Row label="Required" hint="Sets the required field flag">
          <input name="fullName" required aria-label="Full name" tw={`w-[220px] ${field}`} />
        </Row>

        <Row label="Read-only" hint="Visible, not editable">
          <input name="issued" readOnly defaultValue="2026-03-01" tw={`w-[140px] ${field}`} />
        </Row>

        <Row label="Disabled" hint="Read-only and left out of the export">
          <input
            name="reference"
            disabled
            defaultValue="AGR-2026-0413"
            tw={`w-[180px] ${field} bg-[#f3f4f6] text-[#8a9099]`}
          />
        </Row>

        <Row label="Right aligned" hint="text-align reaches the widget text">
          <input name="amount" defaultValue="1450.00" tw={`w-[110px] ${field} text-right`} />
        </Row>

        <Row label="Textarea" hint="Multiline; the text content is the value">
          <textarea name="summary" aria-label="Summary" tw={`h-[54px] w-[300px] ${field} p-2`}>
            Two lines fit here comfortably.
          </textarea>
        </Row>
      </div>

      <div style={{ breakBefore: "page" }} tw="flex flex-col">
        <h2 tw="m-0 mb-1 text-sm font-semibold">Selection controls</h2>

        <Row label="Checkbox" hint="value is the export state, default on">
          <div tw="flex items-center">
            <input
              type="checkbox"
              name="subscribe"
              value="yes"
              defaultChecked
              aria-label="Subscribe"
              tw={`${box} rounded-[2px]`}
            />
            <span tw="text-xs">Subscribe</span>
          </div>
        </Row>

        <Row label="Radio group" hint="One field; same name, different values">
          <div tw="flex">
            {["monthly", "annual"].map((plan) => (
              <div key={plan} tw="mr-5 flex items-center">
                <input
                  type="radio"
                  name="billing"
                  value={plan}
                  defaultChecked={plan === "annual"}
                  aria-label={plan}
                  tw={`${box} rounded-full`}
                />
                <span tw="text-xs capitalize">{plan}</span>
              </div>
            ))}
          </div>
        </Row>

        <Row label="Drop-down" hint="Closed select; label shows, value exports">
          <select name="region" defaultValue="apac" aria-label="Region" tw={`w-[180px] ${field}`}>
            <option value="emea">Europe</option>
            <option value="apac">Asia Pacific</option>
            <option value="amer">Americas</option>
          </select>
        </Row>

        <Row label="List box" hint="size&gt;1; selected row is highlighted">
          <select
            name="tier"
            size={3}
            defaultValue="pro"
            aria-label="Tier"
            tw={`w-[180px] ${field} h-[56px] px-0`}
          >
            <option value="free">Free</option>
            <option value="pro">Pro</option>
            <option value="scale">Scale</option>
          </select>
        </Row>

        <Row label="Multi-select" hint="multiple allows several values">
          <select
            name="addons"
            multiple
            size={3}
            defaultValue={["sso", "audit"]}
            aria-label="Add-ons"
            tw={`w-[180px] ${field} h-[56px] px-0`}
          >
            <option value="sso">SSO</option>
            <option value="audit">Audit log</option>
            <option value="sla">SLA</option>
          </select>
        </Row>

        <Row label="Not fillable" hint="Buttons stay static; no field is made">
          <input type="submit" value="Send" tw="text-xs" />
        </Row>
      </div>
    </div>
  );
}

export const options: PlaygroundOptions = {
  pdf: {
    size: "a4",
    margin: 44,
    form: true,
    metadata: { title: "Fillable form controls", creationDate: "2026-03-01" },
    lang: "en",
  },
};
