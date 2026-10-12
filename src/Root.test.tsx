import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { setCurrentLang } from "./i18n";
import { Root } from "./Root";
import { fakeBackend } from "./test/fakeBackend";

describe("Root", () => {
  beforeEach(() => {
    // inTauri() looks for this; the fake backend answers the commands.
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    };
  });
  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.restoreAllMocks();
    setCurrentLang("vi");
  });

  it("uses the saved language and tells the backend to keep it", async () => {
    fakeBackend.language = "en";
    render(<Root />);
    expect(await screen.findByRole("button", { name: "Undo" })).toBeInTheDocument();
    expect(fakeBackend.calls).toContainEqual({ cmd: "set_language", args: { language: "en", remember: true } });
  });

  it("follows the system language without remembering it", async () => {
    vi.spyOn(navigator, "languages", "get").mockReturnValue(["vi-VN"]);
    render(<Root />);
    expect(await screen.findByRole("button", { name: "Hoàn tác" })).toBeInTheDocument();
    await waitFor(() =>
      expect(fakeBackend.calls).toContainEqual({ cmd: "set_language", args: { language: "vi", remember: false } }),
    );
    expect(fakeBackend.language).toBeNull();
  });
});
