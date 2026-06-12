export interface ThemedHtml {
  light: string;
  dark: string;
}

// Static markup for both themes, toggled by the `.dark` class; avoids
// hydration-mismatched dangerouslySetInnerHTML that React never patches.
export function ShikiHtml({ html, className }: { html: ThemedHtml; className?: string }) {
  return (
    <div
      className={`[&_pre]:bg-transparent! [&_pre]:m-0! [&_pre]:p-0! [&_code]:bg-transparent! ${className ?? ""}`}
    >
      <div className="dark:hidden" dangerouslySetInnerHTML={{ __html: html.light }} />
      <div className="hidden dark:block" dangerouslySetInnerHTML={{ __html: html.dark }} />
    </div>
  );
}
