import type { Command, KeyBinding } from "@codemirror/view";
import { vscodeKeymap } from "@replit/codemirror-vscode-keymap";
import { openReplacePanel } from "./find-widget";

export type PlaygroundActions = { run: Command; format: Command };

/** VS Code's bindings plus the playground's own. */
export function editorBindings(actions: PlaygroundActions): readonly KeyBinding[] {
  return [
    { key: "Mod-Enter", run: actions.run, preventDefault: true },
    { key: "Shift-Alt-f", run: actions.format, preventDefault: true },
    { key: "Mod-Alt-f", run: openReplacePanel, preventDefault: true },
    ...vscodeKeymap,
  ];
}
