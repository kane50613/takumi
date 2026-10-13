import { Link } from "waku";
import { Button } from "~/components/ui/button";
import { H2, LINE, LINK, VALUE } from "./typography";

export function CTA() {
  return (
    <section className="relative z-10 px-6 pt-12 pb-36 lg:px-[max(6vw,2rem)]">
      <div className="space-y-4 lg:w-[26rem]">
        <h2 className={H2}>Render your first image.</h2>
        <p className={LINE}>Install the package, then read the quick start.</p>
        <p className="font-mono text-sm select-all">bun i takumi-js</p>
        <div className="flex items-center gap-6 pt-2">
          <Button asChild size="lg" className="px-7 font-semibold active:scale-[0.97]">
            <Link to="/docs">Get started</Link>
          </Button>
          <a
            href="https://github.com/kane50613/takumi"
            target="_blank"
            rel="noopener noreferrer"
            className={`${LINK} text-sm`}
          >
            GitHub
          </a>
        </div>
        <p className={`${VALUE} pt-6`}>
          Layout by Taffy · text by Parley and Skrifa · MIT or Apache-2.0
        </p>
      </div>
    </section>
  );
}
