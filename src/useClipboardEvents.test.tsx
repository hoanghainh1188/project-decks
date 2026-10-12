import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useClipboardEvents } from "./useClipboardEvents";

function Harness({ dispatch }: { dispatch: (id: string) => void }) {
  useClipboardEvents(dispatch, () => "Một\nHai");
  return <input aria-label="field" />;
}

function clipboardEvent(type: string) {
  const data = new Map<string, string>();
  const event = new Event(type, { bubbles: true, cancelable: true }) as Event & { clipboardData: unknown };
  event.clipboardData = { setData: (kind: string, value: string) => data.set(kind, value) };
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
});
