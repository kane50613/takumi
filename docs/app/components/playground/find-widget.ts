import {
  closeSearchPanel,
  findNext,
  findPrevious,
  getSearchQuery,
  openSearchPanel,
  replaceAll,
  replaceNext,
  SearchQuery,
  setSearchQuery,
} from "@codemirror/search";
import { type EditorState, StateEffect } from "@codemirror/state";
import {
  type Command,
  EditorView,
  type Panel,
  runScopeHandlers,
  type ViewUpdate,
} from "@codemirror/view";

const showReplace = StateEffect.define<boolean>();

export const openReplacePanel: Command = (view) => {
  openSearchPanel(view);
  view.dispatch({ effects: showReplace.of(true) });

  return true;
};

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string,
  children: (Node | string)[] = [],
) {
  const node = document.createElement(tag);

  node.className = className;
  node.append(...children);

  return node;
}

function field(placeholder: string, value: string, oninput: () => void) {
  const input = element("input", "cm-find-field");

  input.placeholder = placeholder;
  input.value = value;
  input.oninput = oninput;

  return input;
}

function button(className: string, label: string, title: string, onclick: () => void) {
  const node = element("button", className, [label]);

  node.type = "button";
  node.title = title;
  node.onclick = onclick;

  return node;
}

function toggle(label: string, title: string, onChange: () => void) {
  const button = element("button", "cm-find-toggle", [label]);

  button.type = "button";
  button.title = title;
  button.setAttribute("aria-pressed", "false");
  button.addEventListener("click", () => {
    button.setAttribute(
      "aria-pressed",
      button.getAttribute("aria-pressed") === "true" ? "false" : "true",
    );
    onChange();
  });

  return button;
}

function pressed(button: HTMLButtonElement) {
  return button.getAttribute("aria-pressed") === "true";
}

/** A find and replace panel laid out like VS Code's find widget. */
export class FindWidget implements Panel {
  readonly dom: HTMLElement;
  readonly top = true;
  private query: SearchQuery;
  private readonly searchField: HTMLInputElement;
  private readonly replaceField: HTMLInputElement;
  private readonly caseToggle: HTMLButtonElement;
  private readonly wordToggle: HTMLButtonElement;
  private readonly regexpToggle: HTMLButtonElement;
  private readonly count: HTMLElement;
  private readonly replaceRow: HTMLElement;
  private readonly expand: HTMLButtonElement;

  constructor(private readonly view: EditorView) {
    this.query = getSearchQuery(view.state);
    this.searchField = field("Find", this.query.search, () => this.commit());
    this.searchField.setAttribute("main-field", "true");
    this.replaceField = field("Replace", this.query.replace, () => this.commit());
    this.caseToggle = toggle("Aa", "Match Case", () => this.commit());
    this.wordToggle = toggle("ab", "Match Whole Word", () => this.commit());
    this.regexpToggle = toggle(".*", "Use Regular Expression", () => this.commit());
    this.count = element("span", "cm-find-count");
    this.expand = button("cm-find-expand", "", "Toggle Replace", () =>
      this.setReplaceVisible(!this.replaceVisible),
    );

    const action = (label: string, title: string, command: Command) =>
      button("cm-find-action", label, title, () => command(view));

    this.replaceRow = element("div", "cm-find-row cm-find-replace", [
      this.replaceField,
      action("Replace", "Replace", replaceNext),
      action("All", "Replace All", replaceAll),
    ]);
    this.dom = element("div", "cm-find", [
      this.expand,
      element("div", "cm-find-rows", [
        element("div", "cm-find-row", [
          element("div", "cm-find-input", [
            this.searchField,
            this.caseToggle,
            this.wordToggle,
            this.regexpToggle,
          ]),
          this.count,
          action("↑", "Previous Match", findPrevious),
          action("↓", "Next Match", findNext),
          action("×", "Close", closeSearchPanel),
        ]),
        this.replaceRow,
      ]),
    ]);
    this.dom.onkeydown = (event) => this.keydown(event);
    this.syncCount(view.state);
  }

  private get replaceVisible() {
    return this.expand.getAttribute("aria-expanded") === "true";
  }

  private setReplaceVisible(visible: boolean) {
    this.expand.setAttribute("aria-expanded", visible ? "true" : "false");
    this.replaceRow.hidden = !visible;
  }

  mount() {
    this.setReplaceVisible(false);
    this.searchField.select();
  }

  update(update: ViewUpdate) {
    for (const transaction of update.transactions) {
      for (const effect of transaction.effects) {
        if (effect.is(showReplace)) this.setReplaceVisible(effect.value);
      }
    }

    const query = getSearchQuery(update.state);
    const queryChanged = !query.eq(this.query);

    if (queryChanged) {
      this.query = query;
      this.searchField.value = query.search;
      this.replaceField.value = query.replace;
      this.caseToggle.setAttribute("aria-pressed", String(query.caseSensitive));
      this.wordToggle.setAttribute("aria-pressed", String(query.wholeWord));
      this.regexpToggle.setAttribute("aria-pressed", String(query.regexp));
    }

    if (update.docChanged || update.selectionSet || queryChanged) this.syncCount(update.state);
  }

  private commit() {
    const query = new SearchQuery({
      search: this.searchField.value,
      replace: this.replaceField.value,
      caseSensitive: pressed(this.caseToggle),
      wholeWord: pressed(this.wordToggle),
      regexp: pressed(this.regexpToggle),
      literal: true,
    });

    if (query.eq(this.query)) return;

    this.view.dispatch({ effects: setSearchQuery.of(query) });
  }

  private syncCount(state: EditorState) {
    if (!this.query.search || !this.query.valid) {
      this.count.textContent = "";
      return;
    }

    const selection = state.selection.main;
    const matches = this.query.getCursor(state);
    let total = 0;
    let current = 0;

    for (let match = matches.next(); !match.done; match = matches.next()) {
      total += 1;

      if (match.value.from === selection.from && match.value.to === selection.to) current = total;
    }

    this.count.textContent =
      total === 0 ? "No results" : `${current === 0 ? "?" : current} of ${total}`;
    this.count.classList.toggle("cm-find-count-empty", total === 0);
  }

  private keydown(event: KeyboardEvent) {
    if (runScopeHandlers(this.view, event, "search-panel")) {
      event.preventDefault();
      return;
    }

    if (event.key !== "Enter") return;

    event.preventDefault();

    if (event.target === this.replaceField) {
      replaceNext(this.view);
      return;
    }

    (event.shiftKey ? findPrevious : findNext)(this.view);
  }
}

export const findWidgetTheme = EditorView.baseTheme({
  ".cm-panels-top": {
    position: "absolute",
    top: "0",
    right: "20px",
    left: "auto",
    width: "auto",
    zIndex: "10",
    background: "transparent",
    border: "none",
  },
  ".cm-panel.cm-find": {
    display: "flex",
    alignItems: "stretch",
    gap: "2px",
    padding: "4px 6px 4px 0",
    fontSize: "13px",
    lineHeight: "1.4",
    borderRadius: "0 0 6px 6px",
    fontFamily: "var(--font-sans, system-ui)",
  },
  ".cm-find-expand": {
    width: "18px",
    border: "none",
    background: "transparent",
    color: "inherit",
    cursor: "pointer",
    padding: "0",
    "&::before": { content: "'▸'" },
    "&[aria-expanded=true]::before": { content: "'▾'" },
  },
  ".cm-find-rows": { display: "flex", flexDirection: "column", gap: "4px" },
  ".cm-find-row": { display: "flex", alignItems: "center", gap: "4px" },
  ".cm-find-input": {
    display: "flex",
    alignItems: "center",
    gap: "2px",
    padding: "0 2px 0 0",
    borderRadius: "3px",
    border: "1px solid transparent",
    "&:focus-within": { borderColor: "var(--cm-focus-border)" },
  },
  ".cm-find-field": {
    width: "200px",
    padding: "3px 6px",
    border: "1px solid transparent",
    borderRadius: "3px",
    background: "transparent",
    color: "inherit",
    font: "inherit",
    outline: "none",
    "&::placeholder": { color: "var(--cm-placeholder)" },
  },
  ".cm-find-replace .cm-find-field": { borderRadius: "3px" },
  ".cm-find-toggle, .cm-find-action": {
    minWidth: "22px",
    height: "22px",
    padding: "0 4px",
    border: "1px solid transparent",
    borderRadius: "3px",
    background: "transparent",
    color: "inherit",
    font: "inherit",
    cursor: "pointer",
    "&:hover": { background: "var(--cm-hover)" },
  },
  ".cm-find-toggle[aria-pressed=true]": {
    background: "var(--cm-toggle-active)",
    borderColor: "var(--cm-focus-border)",
  },
  ".cm-find-count": {
    minWidth: "70px",
    fontSize: "12px",
    padding: "0 4px",
    "&.cm-find-count-empty": { color: "var(--cm-error)" },
  },
});
