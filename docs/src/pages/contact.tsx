import { ProsePage } from "~/components/prose-page";

const TITLE = "Contact Takumi";
const DESCRIPTION =
  "How to report a bug, ask a question, report a security issue, or reach the maintainer of Takumi.";

export default function Contact() {
  return (
    <ProsePage title={TITLE} heading="Contact" description={DESCRIPTION} path="/contact">
      <p>
        Takumi is a one person project with an open issue tracker. Public channels get answered
        first, because the answer helps whoever searches for it next.
      </p>
      <h2>Bugs and feature requests</h2>
      <p>
        File them at{" "}
        <a href="https://github.com/kane50613/takumi/issues">github.com/kane50613/takumi/issues</a>.
        A reproduction helps most: the template, the options you passed, and what you expected
        instead. A link from the <a href="/playground">playground</a> carries the whole snippet in
        the URL, so it is usually enough on its own.
      </p>
      <h2>Questions and ideas</h2>
      <p>
        Open a thread in{" "}
        <a href="https://github.com/kane50613/takumi/discussions">GitHub Discussions</a> for
        anything that is not a bug: how to lay something out, whether a CSS property is supported,
        or what a future version should do.
      </p>
      <h2>Security</h2>
      <p>
        Report anything that looks exploitable privately by email to{" "}
        <a href="mailto:me@kane.tw">me@kane.tw</a>. Please do not open a public issue for it. Say
        which version and which binding you used, and include the input that triggers the problem.
      </p>
      <h2>Everything else</h2>
      <p>
        Email <a href="mailto:me@kane.tw">me@kane.tw</a>, or find Kane Wang on X at{" "}
        <a href="https://x.com/kanewang_">@kanewang_</a>. The project is based in Taiwan, so replies
        arrive on UTC+8 working hours.
      </p>
    </ProsePage>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
