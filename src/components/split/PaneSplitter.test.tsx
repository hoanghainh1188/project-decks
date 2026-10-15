import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../../i18n";
import { PaneSplitter } from "./PaneSplitter";
import { clampShare, DEFAULT_SHARE, readShare, saveShare } from "./sheetShare";

afterEach(() => window.localStorage.clear());

describe("sheet share", () => {
  it("keeps the share between the limits", () => {
    expect(clampShare(0.05)).toBe(0.15);
    expect(clampShare(0.99)).toBe(0.85);
    expect(clampShare(Number.NaN)).toBe(DEFAULT_SHARE);
  });

  it("remembers the share and falls back to the default", () => {
    expect(readShare()).toBe(DEFAULT_SHARE);
    saveShare(0.4);
    expect(readShare()).toBe(0.4);
    window.localStorage.setItem("sheafly.sheet.share", "nonsense");
    expect(readShare()).toBe(DEFAULT_SHARE);
  });
});

function renderSplitter(share = 0.5) {
  const onChange = vi.fn();
  render(
    <I18nProvider lang="vi">
      <div>
        <PaneSplitter share={share} onChange={onChange} />
      </div>
    </I18nProvider>,
  );
  const separator = screen.getByRole("separator", { name: "Chia bảng công việc và Gantt" });
  return { onChange, separator };
}

describe("PaneSplitter", () => {
  it("is a focusable vertical separator with its value", () => {
    const { separator } = renderSplitter(0.5);
    expect(separator).toHaveAttribute("aria-orientation", "vertical");
    expect(separator).toHaveAttribute("aria-valuenow", "50");
    expect(separator).toHaveAttribute("tabindex", "0");
  });

  it("moves with the arrow keys and goes back to the default on Home or a double click", () => {
    const { onChange, separator } = renderSplitter(0.5);
    fireEvent.keyDown(separator, { key: "ArrowLeft" });
    expect(onChange).toHaveBeenLastCalledWith(0.48);
    fireEvent.keyDown(separator, { key: "ArrowRight" });
    expect(onChange).toHaveBeenLastCalledWith(0.52);
    fireEvent.keyDown(separator, { key: "ArrowLeft", shiftKey: true });
    expect(onChange).toHaveBeenLastCalledWith(0.4);
    fireEvent.keyDown(separator, { key: "End" });
    expect(onChange).toHaveBeenLastCalledWith(0.85);
    fireEvent.keyDown(separator, { key: "Home" });
    expect(onChange).toHaveBeenLastCalledWith(DEFAULT_SHARE);
    fireEvent.doubleClick(separator);
    expect(onChange).toHaveBeenLastCalledWith(DEFAULT_SHARE);
  });

  it("follows the pointer across its parent", () => {
    const { onChange, separator } = renderSplitter(0.5);
    const parent = separator.parentElement!;
    vi.spyOn(parent, "getBoundingClientRect").mockReturnValue({ left: 100, width: 1000, top: 0, right: 1100, bottom: 0, height: 0, x: 100, y: 0, toJSON: () => ({}) });
    fireEvent.pointerDown(separator, { clientX: 600, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 400, buttons: 1, pointerId: 1 });
    expect(onChange).toHaveBeenLastCalledWith(0.3);
    fireEvent.pointerUp(window, { pointerId: 1 });
    onChange.mockClear();
    fireEvent.pointerMove(window, { clientX: 800, buttons: 1, pointerId: 1 });
    expect(onChange).not.toHaveBeenCalled();
  });

  it("stops when the button was let go outside the window", () => {
    const { onChange, separator } = renderSplitter(0.5);
    const parent = separator.parentElement!;
    vi.spyOn(parent, "getBoundingClientRect").mockReturnValue({ left: 0, width: 1000, top: 0, right: 1000, bottom: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) });
    fireEvent.pointerDown(separator, { clientX: 500, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 300, buttons: 0, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 200, buttons: 1, pointerId: 1 });
    expect(onChange).not.toHaveBeenCalled();
    expect(document.body).not.toHaveClass("pane-resizing");
  });
});
