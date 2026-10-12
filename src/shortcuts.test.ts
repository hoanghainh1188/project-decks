import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  commitFocusedField,
  hasUncommittedText,
  isEditableTarget,
  lineStartRange,
  shortcutId,
  shouldRunAction,
  trackCommittedText,
} from "./shortcuts";

function keyEvent(key: string, init: Partial<KeyboardEventInit> = {}): KeyboardEvent {
  return new KeyboardEvent("keydown", { key, metaKey: true, ...init });
}

describe("shortcutId", () => {
  it.each([
    ["z", {}, "undo"],
    ["Z", { shiftKey: true }, "redo"],
    ["s", {}, "save"],
    ["s", { shiftKey: true }, "save-as"],
    ["o", {}, "open"],
    ["n", {}, "new"],
    ["]", {}, "indent"],
    ["[", {}, "outdent"],
    ["Backspace", {}, "delete"],
    ["d", {}, "fill-down"],
    ["ç", { altKey: true, code: "KeyC" }, "copy-task"],
    ["√", { altKey: true, code: "KeyV" }, "paste-task"],
  ])("maps Cmd+%s to %s", (key, init, id) => {
    expect(shortcutId(keyEvent(key, init))).toBe(id);
  });

  it("accepts Ctrl as the modifier", () => {
    expect(shortcutId(keyEvent("z", { metaKey: false, ctrlKey: true }))).toBe("undo");
  });

  it("ignores keys without a modifier and unknown keys", () => {
    expect(shortcutId(keyEvent("z", { metaKey: false }))).toBeNull();
    expect(shortcutId(keyEvent("Backspace", { metaKey: false }))).toBeNull();
    expect(shortcutId(keyEvent("k"))).toBeNull();
  });
});

describe("isEditableTarget", () => {
  it.each([
    ["input", () => document.createElement("input")],
    ["textarea", () => document.createElement("textarea")],
  ])("treats %s as editable", (_name, make) => {
    expect(isEditableTarget(make())).toBe(true);
  });

  it("treats contenteditable elements as editable", () => {
    const div = document.createElement("div");
    div.setAttribute("contenteditable", "true");
    document.body.append(div);
    expect(isEditableTarget(div)).toBe(true);
    div.remove();
  });

  it("does not treat selects, buttons, checkboxes or the page as editable", () => {
    expect(isEditableTarget(document.createElement("select"))).toBe(false);
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    expect(isEditableTarget(checkbox)).toBe(false);
    expect(isEditableTarget(document.createElement("button"))).toBe(false);
    expect(isEditableTarget(document.body)).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });
});

describe("committed text tracking", () => {
  let stop: () => void;
  let field: HTMLInputElement;

  beforeEach(() => {
    stop = trackCommittedText(document);
    field = document.createElement("input");
    field.value = "Thiết kế";
    document.body.append(field);
    field.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
  });

  afterEach(() => {
    field.remove();
    stop();
  });

  it("starts clean when the field gets focus", () => {
    expect(hasUncommittedText(field)).toBe(false);
  });

  it("sees typing as uncommitted", () => {
    field.value = "Thiết kế UI";
    expect(hasUncommittedText(field)).toBe(true);
  });

  it("treats a change event as a commit", () => {
    field.value = "Thiết kế UI";
    field.dispatchEvent(new Event("change", { bubbles: true }));
    expect(hasUncommittedText(field)).toBe(false);
  });

  it("treats Enter as a commit", () => {
    field.value = "Thiết kế UI";
    field.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(hasUncommittedText(field)).toBe(false);
  });

  it("ignores fields it never saw get focus", () => {
    const other = document.createElement("input");
    other.value = "ab";
    expect(hasUncommittedText(other)).toBe(false);
  });
});

describe("shouldRunAction", () => {
  let stop: () => void;
  let field: HTMLInputElement;
  const button = document.createElement("button");

  beforeEach(() => {
    stop = trackCommittedText(document);
    field = document.createElement("input");
    field.value = "1 ngày";
    document.body.append(field);
    field.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
  });

  afterEach(() => {
    field.remove();
    stop();
  });

  it.each(["undo", "redo"])("leaves %s to a field with uncommitted text", (id) => {
    field.value = "3 ngày";
    expect(shouldRunAction(id, field)).toBe(false);
  });

  it.each(["undo", "redo"])("runs project %s when the focused field has nothing pending", (id) => {
    expect(shouldRunAction(id, field)).toBe(true);
    expect(shouldRunAction(id, button)).toBe(true);
  });

  it("never deletes the task from inside a text field", () => {
    expect(shouldRunAction("delete", field)).toBe(false);
    expect(shouldRunAction("delete", button)).toBe(true);
  });

  it.each(["save", "save-as", "open", "new", "indent", "outdent", "view-gantt"])(
    "still runs %s while a field has focus",
    (id) => {
      field.value = "3 ngày";
      expect(shouldRunAction(id, field)).toBe(true);
    },
  );
});

describe("lineStartRange", () => {
  it("deletes back to the start of a single line", () => {
    expect(lineStartRange("Thiết kế UI", 7, 7)).toEqual([0, 7]);
  });

  it("stops at the previous line break", () => {
    expect(lineStartRange("dòng một\ndòng hai", 13, 13)).toEqual([9, 13]);
  });

  it("deletes the selection when there is one", () => {
    expect(lineStartRange("Thiết kế UI", 2, 6)).toEqual([2, 6]);
  });

  it("does nothing at the start of a line", () => {
    expect(lineStartRange("dòng một\ndòng hai", 9, 9)).toEqual([9, 9]);
  });
});

describe("shouldRunAction behind a modal dialog", () => {
  afterEach(() => document.body.replaceChildren());

  it("keeps only undo, redo and save while a modal dialog is open", () => {
    const button = document.createElement("button");
    document.body.append(button);
    expect(shouldRunAction("delete", button)).toBe(true);
    const dialog = document.createElement("div");
    dialog.setAttribute("aria-modal", "true");
    document.body.append(dialog);
    for (const id of ["delete", "indent", "open", "view-gantt", "project-info"]) {
      expect(shouldRunAction(id, button)).toBe(false);
    }
    for (const id of ["undo", "redo", "save", "save-as"]) expect(shouldRunAction(id, button)).toBe(true);
  });
});

describe("commitFocusedField", () => {
  afterEach(() => document.body.replaceChildren());

  it("blurs a text field so its onBlur commit runs, and leaves buttons alone", () => {
    const input = document.createElement("input");
    const committed: string[] = [];
    input.addEventListener("blur", () => committed.push(input.value));
    document.body.append(input);
    input.focus();
    input.value = "Tên mới";
    commitFocusedField();
    expect(committed).toEqual(["Tên mới"]);
    expect(input).not.toHaveFocus();

    const button = document.createElement("button");
    document.body.append(button);
    button.focus();
    commitFocusedField();
    expect(button).toHaveFocus();
  });
});
