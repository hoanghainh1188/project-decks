import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useClipboardEvents } from "./useClipboardEvents";

function Harness({ dispatch, pasteText }: { dispatch: (id: string) => void; pasteText?: (text: string) => boolean }) {
  useClipboardEvents(dispatch, () => "Một\nHai", pasteText);
  return <input aria-label="field" />;
}

function clipboardEvent(type: string, text = "") {
  const data = new Map<string, string>([["text/plain", text]]);
  const event = new Event(type, { bubbles: true, cancelable: true }) as Event & { clipboardData: unknown };
  event.clipboardData = {
    setData: (kind: string, value: string) => data.set(kind, value),
    getData: (kind: string) => data.get(kind) ?? "",
  };
  return { event, data };
}

describe("useClipboardEvents", () => {
  afterEach(() => {
    (document.activeElement as HTMLElement | null)?.blur();
  });

  it("copies and pastes tasks when no text field has focus", () => {
    const dispatch = vi.fn();
    render(<Harness dispatch={dispatch} />);
    const copy = clipboardEvent("copy");
    document.body.dispatchEvent(copy.event);
    expect(copy.event.defaultPrevented).toBe(true);
    expect(copy.data.get("text/plain")).toBe("Một\nHai");
    document.body.dispatchEvent(clipboardEvent("paste").event);
    expect(dispatch.mock.calls).toEqual([["copy-task"], ["paste-task"]]);
  });

  it("leaves copy and paste to a focused text field", () => {
    const dispatch = vi.fn();
    const { getByLabelText } = render(<Harness dispatch={dispatch} />);
    const field = getByLabelText("field");
    field.focus();
    const copy = clipboardEvent("copy");
    field.dispatchEvent(copy.event);
    expect(copy.event.defaultPrevented).toBe(false);
    expect(dispatch).not.toHaveBeenCalled();
  });

  it("enables the native items outside a field", () => {
    render(<Harness dispatch={vi.fn()} />);
    const before = new Event("beforepaste", { cancelable: true });
    document.dispatchEvent(before);
    expect(before.defaultPrevented).toBe(true);
  });

  it("hands text from another app to pasteText, and pastes tasks when it declines", () => {
    const dispatch = vi.fn();
    const pasteText = vi.fn((text: string) => text.includes("\t"));
    render(<Harness dispatch={dispatch} pasteText={pasteText} />);
    document.body.dispatchEvent(clipboardEvent("paste", "Tên\tThời lượng").event);
    expect(pasteText).toHaveBeenCalledWith("Tên\tThời lượng");
    expect(dispatch).not.toHaveBeenCalled();
    document.body.dispatchEvent(clipboardEvent("paste", "chỉ một tên").event);
    expect(dispatch.mock.calls).toEqual([["paste-task"]]);
  });

  it("pastes tasks when the text is what this app copied", () => {
    const dispatch = vi.fn();
    const pasteText = vi.fn(() => true);
    render(<Harness dispatch={dispatch} pasteText={pasteText} />);
    document.body.dispatchEvent(clipboardEvent("copy").event);
    document.body.dispatchEvent(clipboardEvent("paste", "Một\nHai").event);
    expect(pasteText).not.toHaveBeenCalled();
    expect(dispatch.mock.calls).toEqual([["copy-task"], ["paste-task"]]);
  });
});
