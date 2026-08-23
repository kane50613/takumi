import { ProsePage } from "~/components/prose-page";

const TITLE = "Privacy at Takumi";
const DESCRIPTION =
  "What takumi.kane.tw collects, what stays in your browser, and which third parties the site depends on.";

export default function Privacy() {
  return (
    <ProsePage title={TITLE} heading="Privacy" description={DESCRIPTION} path="/privacy">
      <p>
        This site is documentation for an open source library. It has no accounts, no sign in, and
        no shopping cart, so there is nothing here to register for and no profile to delete.
      </p>
      <h2>What the site measures</h2>
      <p>
        Page views are counted with Vercel Web Analytics. It records the page, the referrer, the
        country, and the kind of device, and it does not set a tracking cookie or follow you to
        other sites. Nothing on this site is used for advertising and nothing is sold.
      </p>
      <h2>The playground</h2>
      <p>
        The playground uses WebAssembly to compile and render your code in the browser. It does not
        upload your code to a server. The share button compresses your code into the URL fragment.
        Browsers do not send a fragment to the server, but anyone holding the link can read the
        code. The playground also keeps one flag in local storage to remember that you have already
        seen the hint about running a template.
      </p>
      <h2>Third parties</h2>
      <p>
        The site is hosted by Vercel, which logs requests as part of running the service. Fonts are
        loaded from Google Fonts, so a font request reveals your IP address to Google. The showcase
        page loads each project's preview image from that project's own domain. Documentation search
        runs in your browser against an index downloaded from this site, and your queries are not
        sent anywhere.
      </p>
      <h2>The library itself</h2>
      <p>
        Takumi is a rendering engine that runs on your own machines. It sends no telemetry and needs
        no key. It reads only the fonts and images you hand it.
      </p>
      <h2>Questions</h2>
      <p>
        Email <a href="mailto:me@kane.tw">me@kane.tw</a> with anything about this page or about data
        the site holds. This page was last updated on 23 August 2026.
      </p>
    </ProsePage>
  );
}

export async function getConfig() {
  return {
    render: "static" as const,
  };
}
