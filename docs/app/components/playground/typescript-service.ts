import { autocompletion } from "@codemirror/autocomplete";
import { forceLinting } from "@codemirror/lint";
import { StateEffect, StateField, type Extension } from "@codemirror/state";
import {
  EditorView,
  showTooltip,
  ViewPlugin,
  type Tooltip,
  type ViewUpdate,
} from "@codemirror/view";
import {
  tsAutocompleteWorker,
  tsFacetWorker,
  tsHoverWorker,
  tsLinterWorker,
  tsSyncWorker,
} from "@valtown/codemirror-ts";
import * as Comlink from "comlink";
import type { SignatureHelp, TypeScriptWorker } from "./typescript.worker";

/** The path the snippet takes inside the virtual file system; `paths` resolve against its root. */
const EDITOR_PATH = "/index.tsx";

type TypeScriptWorkerHandle = Comlink.Remote<TypeScriptWorker>;

export type TypeScriptService = {
  extensions: Extension;
  dispose: () => void;
};

/** Loads the typings the document needs: the core set once, echarts only when it mentions it. */
function typingsLoader(worker: TypeScriptWorkerHandle) {
  return ViewPlugin.fromClass(
    class {
      private readonly requested = new Set<string>();

      constructor(view: EditorView) {
        this.load(view);
      }

      update(update: ViewUpdate) {
        if (update.docChanged) this.load(update.view);
      }

      private load(view: EditorView) {
        this.request(view, "core", () => worker.loadTypings());

        if (view.state.doc.toString().includes("echarts")) {
          this.request(view, "echarts", () => worker.loadEchartsTypings());
        }
      }

      private request(view: EditorView, name: string, load: () => Promise<void>) {
        if (this.requested.has(name)) return;

        this.requested.add(name);
        load()
          .then(() => forceLinting(view))
          .catch(() => this.requested.delete(name));
      }
    },
  );
}

function renderSignature(help: SignatureHelp) {
  const dom = document.createElement("div");

  dom.className = "cm-signature-help";
  dom.append(help.prefix);
  help.parameters.forEach((parameter, index) => {
    if (index > 0) dom.append(help.separator);

    const span = dom.appendChild(document.createElement("span"));

    span.textContent = parameter;
    if (index === help.argumentIndex) span.className = "cm-signature-help-active";
  });
  dom.append(help.suffix);

  return { dom };
}

const setSignatureHelp = StateEffect.define<Tooltip | null>();

const signatureHelpField = StateField.define<Tooltip | null>({
  create: () => null,
  update(tooltip, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setSignatureHelp)) return effect.value;
    }

    if (!tooltip) return null;

    return { ...tooltip, pos: transaction.changes.mapPos(tooltip.pos) };
  },
  provide: (field) => showTooltip.from(field),
});

/** The argument list of the call the cursor sits in, which CodeMirror has no equivalent for. */
function signatureHelp(worker: TypeScriptWorkerHandle) {
  return [
    signatureHelpField,
    ViewPlugin.fromClass(
      class {
        update(update: ViewUpdate) {
          if (!update.docChanged && !update.selectionSet) return;

          const pos = update.state.selection.main.head;

          worker
            .getSignatureHelp({ path: EDITOR_PATH, pos })
            .then((help) => {
              update.view.dispatch({
                effects: setSignatureHelp.of(
                  help ? { pos, above: true, create: () => renderSignature(help) } : null,
                ),
              });
            })
            .catch(() => {});
        }
      },
    ),
    EditorView.baseTheme({
      ".cm-signature-help": { padding: "2px 6px", fontFamily: "var(--font-mono)" },
      ".cm-signature-help-active": { fontWeight: "bold", textDecoration: "underline" },
    }),
  ];
}

async function startWorker() {
  const instance = new Worker(new URL("./typescript.worker.ts", import.meta.url), {
    type: "module",
  });
  const worker: TypeScriptWorkerHandle = Comlink.wrap(instance);

  await worker.initialize();

  return {
    extensions: [
      tsFacetWorker.of({ worker, path: EDITOR_PATH }),
      tsSyncWorker(),
      tsLinterWorker(),
      tsHoverWorker(),
      autocompletion({ override: [tsAutocompleteWorker()] }),
      typingsLoader(worker),
      signatureHelp(worker),
    ],
    stop: () => {
      worker[Comlink.releaseProxy]();
      instance.terminate();
    },
  };
}

/** The layout mounts an editor per breakpoint, and one worker is enough for both. */
let shared: { started: ReturnType<typeof startWorker>; editors: number } | undefined;

/** Starts the language service in a worker and returns the extensions that talk to it. */
export async function startTypeScriptService(): Promise<TypeScriptService> {
  const current = (shared ??= { started: startWorker(), editors: 0 });

  current.editors += 1;

  try {
    const { extensions } = await current.started;

    return {
      extensions,
      dispose: () => {
        current.editors -= 1;

        if (current.editors > 0) return;

        if (shared === current) shared = undefined;
        current.started.then(({ stop }) => stop()).catch(() => {});
      },
    };
  } catch (error) {
    if (shared === current) shared = undefined;
    throw error;
  }
}
